"""WGSL storage records and CPU packing from native layout metadata."""

from __future__ import annotations

import math
import struct
from collections.abc import Iterable
from dataclasses import dataclass, field, fields, is_dataclass
from typing import ClassVar, cast, dataclass_transform, get_origin, get_type_hints

from .. import _pybevy  # pyright: ignore[reportAttributeAccessIssue]
from ..math import Vec2, Vec3, Vec4
from ._layout import _validate_wgsl_identifier
from ._scalars import U32

_LAYOUT_ATTRIBUTE = "_pybevy_shader_type_layout"
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
_TYPES: dict[type, str] = {
    float: "f32",
    int: "i32",
    Vec2: "vec2<f32>",
    Vec3: "vec3<f32>",
    Vec4: "vec4<f32>",
}
_VECTORS: dict[str, tuple[type, str]] = {
    "vec2<f32>": (Vec2, "xy"),
    "vec3<f32>": (Vec3, "xyz"),
    "vec4<f32>": (Vec4, "xyzw"),
}


@dataclass(frozen=True, slots=True)
class ShaderTypeField:
    """One WGSL member's native byte layout, independent of its producer."""

    __module__ = "pybevy.material"
    name: str
    wgsl_type: str
    offset: int
    size: int
    alignment: int


@dataclass(frozen=True, slots=True)
class ShaderTypeLayout:
    """Immutable storage layout usable by CPU packing or future GPU producers."""

    __module__ = "pybevy.material"
    wgsl_type: str
    declarations: str
    size: int
    stride: int
    alignment: int
    fields: tuple[ShaderTypeField, ...]


@dataclass_transform(field_specifiers=(field,))
def shader_type[Record](cls: type[Record]) -> type[Record]:
    """Generate a dataclass and describe its scalar/vector WGSL storage layout."""
    if not is_dataclass(cls) or "__dataclass_fields__" not in cls.__dict__:
        cls = dataclass(cls)
    _validate_wgsl_identifier(cls.__name__, "shader struct")
    if cls.__name__ in _WGSL_BUILTIN_TYPES:
        raise TypeError(f"Shader struct '{cls.__name__}' shadows a WGSL builtin type; rename it")
    hints = get_type_hints(cls, include_extras=True)
    assert is_dataclass(cls)
    members = fields(cls)
    names = {field.name for field in members}
    extra = [
        name
        for name, hint in hints.items()
        if name not in names and get_origin(hint) is not ClassVar
    ]
    if extra:
        raise TypeError(
            f"Shader structs do not support InitVar or undeclared dataclass fields: {', '.join(extra)}"
        )
    if not members:
        raise TypeError("Shader structs must have at least one field")
    wgsl_types: dict[str, str] = {}
    for member in members:
        _validate_wgsl_identifier(member.name, "shader struct field")
        # Naga Oil rewrites trailing digits in struct members during composition.
        if member.name[-1].isdigit():
            raise TypeError(
                f"Shader struct field '{member.name}' must not end in a digit: WGSL shader composition cannot round-trip it; end with a letter or underscore"
            )
        annotation = hints[member.name]
        wgsl_type: str | None
        if annotation == U32:
            wgsl_type = "u32"
        else:
            wgsl_type = _TYPES.get(annotation) if isinstance(annotation, type) else None
        if wgsl_type is None:
            raise TypeError(
                f"Shader struct field '{member.name}' must be float, int, U32, Vec2, Vec3 or Vec4"
            )
        wgsl_types[member.name] = wgsl_type
    declarations = (
        f"struct {cls.__name__} {{\n"
        + "".join(
            f"    {name}: {wgsl_type},\n" for name, wgsl_type in wgsl_types.items()
        )
        + "}"
    )
    size, stride, alignment, native_fields = cast(
        tuple[int, int, int, list[tuple[str, int, int, int]]],
        _pybevy.pbr.StorageShaderMaterial._storage_struct_layout(
            cls.__name__, declarations
        ),
    )
    layout = ShaderTypeLayout(
        cls.__name__,
        declarations,
        size,
        stride,
        alignment,
        tuple(
            ShaderTypeField(
                name, wgsl_types[name], offset, field_size, field_alignment
            )
            for name, offset, field_size, field_alignment in native_fields
        ),
    )
    setattr(cls, _LAYOUT_ATTRIBUTE, layout)
    return cls


def shader_type_layout(cls: type[object]) -> ShaderTypeLayout:
    """Return the native layout for this exact decorated class, without allocating a buffer."""
    layout = cls.__dict__.get(_LAYOUT_ATTRIBUTE)
    if not isinstance(layout, ShaderTypeLayout):
        raise TypeError(f"{cls.__name__} must be decorated with @shader_type")
    return layout


def _float(value: object, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise TypeError(f"{label} must be a float or int")
    try:
        result = float(value)
    except OverflowError as error:
        raise ValueError(f"{label} must be finite and fit f32") from error
    if not math.isfinite(result):
        raise ValueError(f"{label} must be finite and fit f32")
    return result


def _pack_field(
    data: bytearray, offset: int, field: ShaderTypeField, value: object, label: str
) -> None:
    if field.wgsl_type in ("i32", "u32"):
        if type(value) is not int:
            raise TypeError(f"{label} must be an int")
        unsigned = field.wgsl_type == "u32"
        lower, upper = (0, 2**32) if unsigned else (-(2**31), 2**31)
        if not lower <= value < upper:
            raise ValueError(f"{label} must fit {field.wgsl_type}")
        struct.pack_into("<I" if unsigned else "<i", data, offset, value)
        return
    if field.wgsl_type == "f32":
        values = [_float(value, label)]
    else:
        vector_type, axes = _VECTORS[field.wgsl_type]
        if not isinstance(value, vector_type):
            raise TypeError(f"{label} must be {vector_type.__name__}")
        values = [_float(getattr(value, axis), f"{label}.{axis}") for axis in axes]
    try:
        struct.pack_into(f"<{len(values)}f", data, offset, *values)
    except (OverflowError, struct.error) as error:
        raise ValueError(f"{label} must be finite and fit f32") from error


def pack_shader_data(
    values: Iterable[object], *, element: object = None
) -> bytes:
    """Pack homogeneous storage elements; explicit element is required for empty input."""
    inferred = element is None
    records = iter(values)
    first = next(records, _EMPTY)
    if element is None:
        if first is _EMPTY:
            raise TypeError("Empty shader data requires element= (for example element=U32)")
        element = type(first)
    layout: ShaderTypeLayout | None = None
    scalar = "u32" if element == U32 else _TYPES.get(element) if isinstance(element, type) else None
    if scalar is None:
        if not isinstance(element, type):
            raise TypeError("element must be float, int, U32, Vec2/3/4 or a @shader_type class")
        try:
            layout = shader_type_layout(element)
        except TypeError as error:
            raise TypeError(
                f"Unsupported shader element type {element.__name__}; supported: "
                "float, int, Vec2/3/4, and @shader_type records "
                "(use element=U32 for unsigned integers). "
                "Record classes must be decorated with @shader_type."
            ) from error
        stride = layout.stride
    else:
        stride, alignment = cast(
            tuple[int, int],
            _pybevy.pbr.StorageShaderMaterial._storage_element_layout(scalar, ""),
        )
        field = ShaderTypeField("value", scalar, 0, stride, alignment)
    data = bytearray()
    def append(record: object, index: int) -> None:
        start = len(data)
        data.extend(bytes(stride))
        if layout is not None:
            if type(record) is not element:
                assert isinstance(element, type)
                raise TypeError(f"Record {index} must have exact type {element.__name__}")
            for member in layout.fields:
                _pack_field(data, start + member.offset, member, getattr(record, member.name),
                            f"Record {index} {layout.wgsl_type}.{member.name}")
        else:
            if inferred and scalar == "i32" and isinstance(record, float):
                raise TypeError(
                    f"Element {index} must be an int: inferred i32 from the first value; "
                    "use element=float for a floating-point array"
                )
            _pack_field(data, start, field, record, f"Element {index}")
    if first is not _EMPTY:
        append(first, 0)
        for index, record in enumerate(records, 1):
            append(record, index)
    return bytes(data)


_EMPTY = object()
