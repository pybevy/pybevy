"""std140 layout computation and WESL bindings generation for the @material decorator.

Internal machinery for pybevy.material; not part of the public API.
"""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from types import NoneType, UnionType
from typing import TYPE_CHECKING, Any, Union, get_args, get_origin

from ..assets import Handle
from ..color import LinearRgba
from ..image import Image
from ..math import Vec2, Vec3, Vec4

if TYPE_CHECKING:
    from ._bindings import TextureBinding


_MAX_TEXTURE_SLOTS = 4
_UNION_ORIGINS = (Union, UnionType)
_TEXTURE_ANNOTATION = "Handle[Image] | None"

# Keep aligned with Naga's WGSL reserved tokens for the pinned renderer.
_WGSL_RESERVED_NAMES = frozenset(
    """
    alias break case const const_assert continue continuing default diagnostic
    discard else enable false fn for if let loop override requires return struct
    switch true var while
    NULL Self abstract active alignas alignof as asm asm_fragment async attribute
    auto await become cast catch class co_await co_return co_yield coherent
    column_major common compile compile_fragment concept const_cast consteval
    constexpr constinit crate debugger decltype delete demote demote_to_helper
    do dynamic_cast enum explicit export extends extern external fallthrough
    filter final finally friend from fxgroup get goto groupshared highp impl
    implements import inline instanceof interface layout lowp macro macro_rules
    match mediump meta mod module move mut mutable namespace new nil noexcept
    noinline nointerpolation non_coherent noncoherent noperspective null nullptr
    of operator package packoffset partition pass patch pixelfragment precise
    precision premerge priv protected pub public readonly ref regardless register
    reinterpret_cast require resource restrict self set shared sizeof smooth
    snorm static static_assert static_cast std subroutine super target template
    this thread_local throw trait try type typedef typeid typename typeof union
    unless unorm unsafe unsized use using varying virtual volatile wgsl where
    with writeonly yield
    """.split()  # noqa: SIM905
)


_WGSL_BUILTIN_TYPES = frozenset({
    "array", "atomic", "bool", "f16", "f32", "f64", "i32", "i64", "u32", "u64",
    "ptr", "sampler", "sampler_comparison", "binding_array",
    "acceleration_structure", "ray_query", "RayDesc", "RayIntersection",
    "coop_mat8x8", "coop_mat16x16", "texture_external",
    *(f"vec{n}{suffix}" for n in (2, 3, 4) for suffix in ("", "f", "h", "i", "u")),
    *(f"mat{n}x{m}{suffix}" for n in (2, 3, 4) for m in (2, 3, 4) for suffix in ("", "f", "h")),
    *(f"texture_{dimension}" for dimension in ("1d", "2d", "2d_array", "3d", "cube", "cube_array", "multisampled_2d")),
    *(f"texture_depth_{dimension}" for dimension in ("2d", "2d_array", "cube", "cube_array", "multisampled_2d")),
    *(f"texture_storage_{dimension}" for dimension in ("1d", "2d", "2d_array", "3d")),
})


def _validate_wgsl_identifier(name: str, kind: str) -> None:
    if name in _WGSL_RESERVED_NAMES:
        raise TypeError(
            f"Material {kind} '{name}' is a WGSL keyword or reserved word; "
            f"rename it (e.g. '{name}_')."
        )
    if not name.isidentifier() or name == "_" or name.startswith("__"):
        raise TypeError(f"Material {kind} '{name}' is not a valid WGSL identifier")


def _validate_binding_identifier(name: str, kind: str) -> None:
    """Module-scope bindings also must not shadow a WGSL type their declaration names."""
    _validate_wgsl_identifier(name, kind)
    if name in _WGSL_BUILTIN_TYPES:
        raise TypeError(
            f"Material {kind} '{name}' shadows the WGSL type '{name}'; "
            f"rename it (e.g. '{name}_')."
        )


# Types: (wgsl_type, size_bytes, align_bytes, num_floats)
_TYPE_MAP: dict[type, tuple[str, int, int, int]] = {
    float: ("f32", 4, 4, 1),
    int: ("i32", 4, 4, 1),
    Vec2: ("vec2<f32>", 8, 8, 2),
    Vec3: ("vec3<f32>", 12, 16, 3),
    Vec4: ("vec4<f32>", 16, 16, 4),
    LinearRgba: ("vec4<f32>", 16, 16, 4),
}


def _align_up(offset: int, alignment: int) -> int:
    return (offset + alignment - 1) & ~(alignment - 1)


class _FieldLayout:
    __slots__ = (
        "default",
        "name",
        "num_floats",
        "offset",
        "python_type",
        "size",
        "wgsl_type",
    )

    def __init__(
        self,
        name: str,
        python_type: type,
        wgsl_type: str,
        offset: int,
        size: int,
        num_floats: int,
        default: Any,  # noqa: ANN401
    ) -> None:
        self.name = name
        self.python_type = python_type
        self.wgsl_type = wgsl_type
        self.offset = offset
        self.size = size
        self.num_floats = num_floats
        self.default = default


def _annotation_name(typ: object) -> str:
    if get_origin(typ) in _UNION_ORIGINS:
        return " | ".join(_annotation_name(arg) for arg in get_args(typ))
    if typ is NoneType:
        return "None"
    return str(getattr(typ, "__name__", repr(typ)))


def _is_texture_annotation(typ: object) -> bool:
    if typ is Handle:
        return True
    return get_origin(typ) in _UNION_ORIGINS and set(get_args(typ)) == {
        Handle,
        NoneType,
    }


def _check_texture_annotation(name: str, typ: object) -> None:
    members = get_args(typ) if get_origin(typ) in _UNION_ORIGINS else (typ,)
    if Image in members:
        raise TypeError(
            f"Material field '{name}' is annotated '{_annotation_name(typ)}', but a texture "
            f"field holds a Handle[Image]; annotate it as "
            f"'{name}: {_TEXTURE_ANNOTATION} = None'"
        )
    if Handle in members:
        raise TypeError(
            f"Unsupported texture field annotation '{_annotation_name(typ)}' for '{name}'; "
            f"annotate it as 'Handle[Image]' or '{_TEXTURE_ANNOTATION}'"
        )


class _BoolFieldInfo:
    """Tracks a bool field that maps to a shader def bit."""

    __slots__ = ("bit_index", "def_name", "default", "name")

    def __init__(self, name: str, bit_index: int, def_name: str, default: bool) -> None:
        self.name = name
        self.bit_index = bit_index
        self.def_name = def_name
        self.default = default


class _TextureFieldInfo:
    """Tracks a Handle[Image] field that maps to a texture slot."""

    __slots__ = ("binding_sampler", "binding_texture", "layout", "name", "slot_index")

    def __init__(self, name: str, slot_index: int) -> None:
        self.name = name
        self.slot_index = slot_index
        self.layout: TextureBinding | None = None
        # Binding numbers must match the Rust AsBindGroup attributes
        self.binding_texture = 101 + slot_index * 2
        self.binding_sampler = 102 + slot_index * 2


def _compute_layout(
    hints: Mapping[str, object],
    defaults: Mapping[str, Any],
) -> tuple[list[_FieldLayout], list[_BoolFieldInfo], list[_TextureFieldInfo]]:
    """Compute std140-compatible layout for material fields.

    Returns (uniform_fields, bool_fields, texture_fields).
    Bool fields become shader defs, Handle[Image] fields become texture slots.
    """
    fields: list[_FieldLayout] = []
    bool_fields: list[_BoolFieldInfo] = []
    texture_fields: list[_TextureFieldInfo] = []
    offset = 0
    digit_field: str | None = None

    for name, typ in hints.items():
        if name.startswith("_"):
            continue

        # Bool fields → shader defs, not uniforms
        if typ is bool:
            if len(bool_fields) >= 32:
                raise TypeError(f"Too many bool fields (max 32): '{name}' is the 33rd")
            default = defaults.get(name, False)
            bool_fields.append(
                _BoolFieldInfo(
                    name=name,
                    bit_index=len(bool_fields),
                    def_name=name.upper(),
                    default=bool(default),
                )
            )
            continue

        if _is_texture_annotation(typ):
            _validate_binding_identifier(name, "field")
            if len(texture_fields) >= _MAX_TEXTURE_SLOTS:
                raise TypeError(
                    f"Too many texture fields (max {_MAX_TEXTURE_SLOTS}): '{name}'"
                )
            texture_fields.append(
                _TextureFieldInfo(
                    name=name,
                    slot_index=len(texture_fields),
                )
            )
            continue

        _check_texture_annotation(name, typ)
        if not isinstance(typ, type) or typ not in _TYPE_MAP:
            raise TypeError(
                f"Unsupported material field type '{_annotation_name(typ)}' for '{name}'. "
                f"Supported: float, int, bool, Vec2, Vec3, Vec4, LinearRgba, "
                f"{_TEXTURE_ANNOTATION}"
            )

        _validate_wgsl_identifier(name, "field")

        # naga escapes a digit-ending member (tint2 -> tint2_) and composition cannot patch the references.
        if digit_field is None and name[-1].isdigit():
            digit_field = name

        wgsl_type, size, align, num_floats = _TYPE_MAP[typ]
        offset = _align_up(offset, align)

        default = defaults.get(name)

        fields.append(
            _FieldLayout(
                name=name,
                python_type=typ,
                wgsl_type=wgsl_type,
                offset=offset,
                size=size,
                num_floats=num_floats,
                default=default,
            )
        )
        offset += size

    total = _align_up(offset, 16)  # struct alignment = max member alignment
    if total > 1024:
        raise TypeError(f"Material struct too large: {total} bytes (max 1024)")

    if digit_field is not None:
        raise TypeError(
            f"Material field '{digit_field}' ends in a digit; WGSL shader composition "
            f"cannot round-trip struct member names ending in a digit. "
            f"Rename it to end in a letter or an underscore (e.g. '{digit_field}_')."
        )

    return fields, bool_fields, texture_fields


def _validate_binding_names(
    class_name: str,
    fields: list[_FieldLayout],
    textures: list[_TextureFieldInfo],
    storage_names: Iterable[str],
    sampler_names: Iterable[str] = (),
) -> None:
    declarations: list[tuple[str, str]] = []
    if fields:
        declarations.extend(
            ((class_name, "uniform type"), ("material", "generated uniform"))
        )
    for texture in textures:
        declarations.append((texture.name, "texture field"))
        if texture.layout is None or texture.layout.sampler_type is not None:
            declarations.append((f"{texture.name}_sampler", "generated sampler"))
    declarations.extend((name, "storage field") for name in storage_names)
    declarations.extend((name, "sampler field") for name in sampler_names)
    declared: dict[str, str] = {}
    for name, kind in declarations:
        if name in declared:
            raise TypeError(
                f"Material {kind} '{name}' conflicts with {declared[name]} in generated WGSL; "
                "rename the conflicting class or resource field"
            )
        declared[name] = kind


def _generate_wgsl(
    class_name: str,
    fields: list[_FieldLayout],
    texture_fields: list[_TextureFieldInfo] | None = None,
) -> str:
    """Generate the WESL module with the material's struct and binding declarations.

    The ``// module`` header names the import path that user shaders use::

        import pybevy::gen::MyMaterial as params;

    to get the struct and binding declarations without duplicating them.
    """
    _validate_wgsl_identifier(class_name, "class name")
    import_path = f"pybevy::gen::{class_name}"
    lines = [
        "// Auto-generated by @material - do not edit",
        f"// module {import_path}",
    ]

    # Uniform struct (only if there are uniform fields)
    if fields:
        lines.append(f"struct {class_name} {{")
        for f in fields:
            lines.append(f"    {f.name}: {f.wgsl_type},")
        lines.append("}")
        lines.append("")
        lines.append(
            f"@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> material: {class_name};"
        )
        lines.append("")

    # TextureBinding/sampler declarations
    if texture_fields:
        for tf in texture_fields:
            lines.append(
                f"@group(constants::MATERIAL_BIND_GROUP) @binding({tf.binding_texture}) "
                f"var {tf.name}: {tf.layout.wgsl_type if tf.layout else 'texture_2d<f32>'};"
            )
            if tf.layout is None or tf.layout.sampler_type is not None:
                lines.append(
                    f"@group(constants::MATERIAL_BIND_GROUP) @binding({tf.binding_sampler}) "
                    f"var {tf.name}_sampler: {tf.layout._wgsl_sampler_type if tf.layout else 'sampler'};"
                )
        lines.append("")

    return "\n".join(lines)
