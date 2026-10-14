"""WGSL storage records and CPU packing from native layout metadata."""

from __future__ import annotations

import math
import struct
from collections.abc import Iterable
from dataclasses import dataclass, fields, is_dataclass
from typing import ClassVar, cast, get_origin, get_type_hints

from .. import _pybevy  # pyright: ignore[reportAttributeAccessIssue]
from ..math import Vec2, Vec3, Vec4
from ._layout import _validate_wgsl_identifier

_LAYOUT_ATTRIBUTE = "_pybevy_shader_struct_layout"
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
class ShaderStructField:
    """One WGSL member's native byte layout, independent of its producer."""

    __module__ = "pybevy.material"
    name: str
    wgsl_type: str
    offset: int
    size: int
    alignment: int


@dataclass(frozen=True, slots=True)
class ShaderStructLayout:
    """Immutable storage layout usable by CPU packing or future GPU producers."""

    __module__ = "pybevy.material"
    wgsl_type: str
    declarations: str
    size: int
    stride: int
    alignment: int
    fields: tuple[ShaderStructField, ...]


def shader_struct[Record](cls: type[Record]) -> type[Record]:
    """Describe an explicitly decorated dataclass as a scalar/vector WGSL struct."""
    if not is_dataclass(cls) or "__dataclass_fields__" not in cls.__dict__:
        raise TypeError("Apply @shader_struct above @dataclass")
    _validate_wgsl_identifier(cls.__name__, "shader struct")
    hints = get_type_hints(cls, include_extras=True)
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
                f"Shader struct field '{member.name}' must not end in a digit"
            )
        annotation = hints[member.name]
        wgsl_type = _TYPES.get(annotation) if isinstance(annotation, type) else None
        if wgsl_type is None:
            raise TypeError(
                f"Shader struct field '{member.name}' must be float, int, Vec2, Vec3 or Vec4"
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
    layout = ShaderStructLayout(
        cls.__name__,
        declarations,
        size,
        stride,
        alignment,
        tuple(
            ShaderStructField(
                name, wgsl_types[name], offset, field_size, field_alignment
            )
            for name, offset, field_size, field_alignment in native_fields
        ),
    )
    setattr(cls, _LAYOUT_ATTRIBUTE, layout)
    return cls


def shader_struct_layout(cls: type[object]) -> ShaderStructLayout:
    """Return the native layout for this exact decorated class, without allocating a buffer."""
    layout = cls.__dict__.get(_LAYOUT_ATTRIBUTE)
    if not isinstance(layout, ShaderStructLayout):
        raise TypeError(f"{cls.__name__} must be decorated with @shader_struct")
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
    data: bytearray, offset: int, field: ShaderStructField, value: object, label: str
) -> None:
    if field.wgsl_type == "i32":
        if type(value) is not int:
            raise TypeError(f"{label} must be an int")
        if not -(2**31) <= value < 2**31:
            raise ValueError(f"{label} must fit i32")
        struct.pack_into("<i", data, offset, value)
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


def pack_shader_structs[Record](
    records: Iterable[Record], *, struct_type: type[Record] | None = None
) -> bytes:
    """Pack homogeneous records into little-endian storage bytes; never upload them."""
    layout = shader_struct_layout(struct_type) if struct_type is not None else None
    data = bytearray()
    for index, record in enumerate(records):
        if struct_type is None:
            struct_type = type(record)
            layout = shader_struct_layout(struct_type)
        if type(record) is not struct_type:
            raise TypeError(
                f"Record {index} must have exact type {struct_type.__name__}"
            )
        assert layout is not None
        start = len(data)
        data.extend(bytes(layout.stride))
        for field in layout.fields:
            _pack_field(
                data,
                start + field.offset,
                field,
                getattr(record, field.name),
                f"Record {index} {layout.wgsl_type}.{field.name}",
            )
    return bytes(data)
