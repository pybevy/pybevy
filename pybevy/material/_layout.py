"""std140 layout computation and WGSL generation for the @material decorator.

Internal machinery for pybevy.material; not part of the public API.
"""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from ..color import LinearRgba
from ..image import Image
from ..math import Vec2, Vec3, Vec4

_MAX_TEXTURE_SLOTS = 4

# Types: (wgsl_type, size_bytes, align_bytes, num_floats)
_TYPE_MAP: dict[type, tuple[str, int, int, int]] = {
    float: ("f32", 4, 4, 1),
    int: ("f32", 4, 4, 1),
    Vec2: ("vec2<f32>", 8, 8, 2),
    Vec3: ("vec3<f32>", 12, 16, 3),
    Vec4: ("vec4<f32>", 16, 16, 4),
    LinearRgba: ("vec4<f32>", 16, 16, 4),
}


def _align_up(offset: int, alignment: int) -> int:
    return (offset + alignment - 1) & ~(alignment - 1)


class _FieldLayout:
    __slots__ = ("default", "name", "num_floats", "offset", "python_type", "size", "wgsl_type")

    def __init__(
        self, name: str, python_type: type, wgsl_type: str,
        offset: int, size: int, num_floats: int, default: Any,  # noqa: ANN401
    ) -> None:
        self.name = name
        self.python_type = python_type
        self.wgsl_type = wgsl_type
        self.offset = offset
        self.size = size
        self.num_floats = num_floats
        self.default = default


class _BoolFieldInfo:
    """Tracks a bool field that maps to a shader def bit."""
    __slots__ = ("bit_index", "def_name", "default", "name")

    def __init__(self, name: str, bit_index: int, def_name: str, default: bool) -> None:
        self.name = name
        self.bit_index = bit_index
        self.def_name = def_name
        self.default = default


class _TextureFieldInfo:
    """Tracks an Image field that maps to a texture slot."""
    __slots__ = ("binding_sampler", "binding_texture", "name", "slot_index")

    def __init__(self, name: str, slot_index: int) -> None:
        self.name = name
        self.slot_index = slot_index
        # Binding numbers must match the Rust AsBindGroup attributes
        self.binding_texture = 101 + slot_index * 2
        self.binding_sampler = 102 + slot_index * 2


def _compute_layout(
    hints: Mapping[str, type], defaults: Mapping[str, Any],
) -> tuple[list[_FieldLayout], list[_BoolFieldInfo], list[_TextureFieldInfo]]:
    """Compute std140-compatible layout for material fields.

    Returns (uniform_fields, bool_fields, texture_fields).
    Bool fields become shader defs, Image fields become texture slots.
    """
    fields: list[_FieldLayout] = []
    bool_fields: list[_BoolFieldInfo] = []
    texture_fields: list[_TextureFieldInfo] = []
    offset = 0

    for name, typ in hints.items():
        if name.startswith("_"):
            continue

        # Bool fields → shader defs, not uniforms
        if typ is bool:
            if len(bool_fields) >= 32:
                raise TypeError(
                    f"Too many bool fields (max 32): '{name}' is the 33rd"
                )
            default = defaults.get(name, False)
            bool_fields.append(_BoolFieldInfo(
                name=name,
                bit_index=len(bool_fields),
                def_name=name.upper(),
                default=bool(default),
            ))
            continue

        # Image fields → texture slots
        if typ is Image:
            if len(texture_fields) >= _MAX_TEXTURE_SLOTS:
                raise TypeError(
                    f"Too many Image fields (max {_MAX_TEXTURE_SLOTS}): '{name}'"
                )
            texture_fields.append(_TextureFieldInfo(
                name=name,
                slot_index=len(texture_fields),
            ))
            continue

        if typ not in _TYPE_MAP:
            raise TypeError(
                f"Unsupported material field type '{typ.__name__}' for '{name}'. "
                f"Supported: float, int, bool, Image, Vec2, Vec3, Vec4, LinearRgba"
            )

        wgsl_type, size, align, num_floats = _TYPE_MAP[typ]
        offset = _align_up(offset, align)

        default = defaults.get(name)

        fields.append(_FieldLayout(
            name=name,
            python_type=typ,
            wgsl_type=wgsl_type,
            offset=offset,
            size=size,
            num_floats=num_floats,
            default=default,
        ))
        offset += size

    total = _align_up(offset, 16)  # struct alignment = max member alignment
    if total > 1024:
        raise TypeError(f"Material struct too large: {total} bytes (max 1024)")

    return fields, bool_fields, texture_fields


def _generate_wgsl(
    class_name: str,
    fields: list[_FieldLayout],
    texture_fields: list[_TextureFieldInfo] | None = None,
) -> str:
    """Generate WGSL struct + binding declarations.

    Includes ``#define_import_path`` so user shaders can::

        #import pybevy::gen::MyMaterial

    to get the struct and binding declarations without duplicating them.
    """
    import_path = f"pybevy::gen::{class_name}"
    lines = [
        "// Auto-generated by @material - do not edit",
        f"#define_import_path {import_path}",
    ]

    # Uniform struct (only if there are uniform fields)
    if fields:
        lines.append(f"struct {class_name} {{")
        for f in fields:
            lines.append(f"    {f.name}: {f.wgsl_type},")
        lines.append("}")
        lines.append("")
        # Material bind group is group 3 (MATERIAL_BIND_GROUP_INDEX)
        lines.append(f"@group(3) @binding(100) var<uniform> material: {class_name};")
        lines.append("")

    # Texture/sampler declarations
    if texture_fields:
        for tf in texture_fields:
            lines.append(
                f"@group(3) @binding({tf.binding_texture}) "
                f"var {tf.name}: texture_2d<f32>;"
            )
            lines.append(
                f"@group(3) @binding({tf.binding_sampler}) "
                f"var {tf.name}_sampler: sampler;"
            )
        lines.append("")

    return "\n".join(lines)
