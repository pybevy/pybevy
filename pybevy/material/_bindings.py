"""Immutable declaration metadata for material storage layouts."""

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Annotated, NamedTuple, get_args, get_origin

from .. import _pybevy
from ..math import Vec2, Vec3, Vec4
from ._layout import _is_texture_annotation, _validate_wgsl_identifier


@dataclass(frozen=True, init=False, slots=True)
class StorageBinding:
    """Declare a read-only array of a WGSL host-shareable element type.

    ``element`` accepts float, int, Vec2/3/4, or WGSL type text. Custom structs
    belong in ``declarations``. Upload bytes with the reported storage stride;
    this metadata does not serialize Python objects or add padding to data.
    """

    __module__ = "pybevy.material"
    element: type | str
    wgsl_type: str
    declarations: str
    stride: int
    alignment: int

    def __init__(self, *, element: type | str = Vec4, declarations: str = "") -> None:
        types = {
            float: "f32",
            int: "i32",
            Vec2: "vec2<f32>",
            Vec3: "vec3<f32>",
            Vec4: "vec4<f32>",
        }
        wgsl_type = element if isinstance(element, str) else types.get(element)
        if wgsl_type is None:
            raise TypeError(
                "StorageBinding element must be float, int, Vec2/3/4, or a WGSL type string"
            )
        stride, alignment = _pybevy.pbr.StorageShaderMaterial._storage_element_layout(
            wgsl_type, declarations
        )
        for name, value in (
            ("element", element),
            ("wgsl_type", wgsl_type),
            ("declarations", declarations),
            ("stride", stride),
            ("alignment", alignment),
        ):
            object.__setattr__(self, name, value)


class _StorageField(NamedTuple):
    name: str
    binding: int
    layout: StorageBinding


def _partition_storage_fields(
    hints: Mapping[str, object],
) -> tuple[dict[str, object], tuple[_StorageField, ...]]:
    ordinary: dict[str, object] = {}
    storage: list[_StorageField] = []
    for name, annotation in hints.items():
        if get_origin(annotation) is Annotated:
            field_type, *metadata = get_args(annotation)
            markers = [value for value in metadata if isinstance(value, StorageBinding)]
            if markers and not name.startswith("_"):
                if len(markers) != 1 or not _is_texture_annotation(field_type):
                    raise TypeError(
                        f"Storage field '{name}' needs Annotated[Handle[ShaderBuffer] | None, StorageBinding()]"
                    )
                _validate_wgsl_identifier(name, "storage field")
                if name == "material":
                    raise TypeError(
                        "Storage field 'material' conflicts with the generated uniform binding"
                    )
                storage.append(_StorageField(name, 109 + len(storage), markers[0]))
                continue
            annotation = field_type
        ordinary[name] = annotation
    declarations = "\n".join(
        dict.fromkeys(field.layout.declarations for field in storage)
    )
    for field in storage:
        _pybevy.pbr.StorageShaderMaterial._storage_element_layout(
            field.layout.wgsl_type, declarations
        )
    return ordinary, tuple(storage)


@dataclass(frozen=True, init=False, slots=True)
class Texture:
    """Declare an Image binding's dimension, sample type and sampler contract.

    Configure actual filtering/addressing and view dimensions on the Image.
    An explicitly annotated texture has no fallback: None leaves it pending.
    """

    __module__ = "pybevy.material"
    dimension: str
    sample_type: str
    filterable: bool
    sampler: str
    wgsl_type: str
    sampler_type: str

    def __init__(
        self,
        *,
        dimension: str = "2d",
        sample_type: str = "float",
        filterable: bool = True,
        sampler: str = "filtering",
    ) -> None:
        wgsl_type, sampler_type = _pybevy.pbr.StorageShaderMaterial._texture_layout(
            dimension, sample_type, filterable, sampler
        )
        for name, value in (
            ("dimension", dimension),
            ("sample_type", sample_type),
            ("filterable", filterable),
            ("sampler", sampler),
            ("wgsl_type", wgsl_type),
            ("sampler_type", sampler_type),
        ):
            object.__setattr__(self, name, value)


def _partition_texture_fields(
    hints: Mapping[str, object],
) -> tuple[dict[str, object], dict[str, Texture]]:
    ordinary: dict[str, object] = {}
    textures: dict[str, Texture] = {}
    for name, annotation in hints.items():
        if get_origin(annotation) is Annotated and not name.startswith("_"):
            field_type, *metadata = get_args(annotation)
            markers = [value for value in metadata if isinstance(value, Texture)]
            if markers:
                if (
                    len(markers) != 1
                    or any(isinstance(value, StorageBinding) for value in metadata)
                    or not _is_texture_annotation(field_type)
                ):
                    raise TypeError(
                        f"Texture field '{name}' needs Annotated[Handle[Image] | None, Texture()]"
                    )
                textures[name] = markers[0]
                annotation = field_type
        ordinary[name] = annotation
    return ordinary, textures
