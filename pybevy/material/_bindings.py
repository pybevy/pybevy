"""Immutable declaration metadata for material storage layouts."""

from __future__ import annotations

import sys
from collections.abc import Mapping
from dataclasses import dataclass
from dataclasses import field as dataclass_field
from types import GenericAlias
from typing import (
    Annotated,
    Generic,
    NamedTuple,
    Protocol,
    cast,
    get_args,
    get_origin,
    overload,
)

if sys.version_info >= (3, 13):
    from typing import TypeVar
else:
    from typing_extensions import TypeVar

from .. import _pybevy  # pyright: ignore[reportAttributeAccessIssue]
from ..assets import Handle
from ..image import Image, ImageSamplerDescriptor
from ..math import Vec2, Vec3, Vec4
from ..pbr import StorageShaderMaterial
from ..render import ShaderBuffer
from ._layout import _is_texture_annotation, _validate_binding_identifier
from ._scalars import U32
from ._structs import shader_type_layout

_DEFAULT_SAMPLER = ImageSamplerDescriptor()
_Element = TypeVar("_Element", default=Vec4)


class _BindingInstance(Protocol):
    _shader_mat: StorageShaderMaterial | None
    _textures: list[Handle[Image] | None]
    _samplers: list[ImageSamplerDescriptor]
    _storage_buffers: list[Handle[ShaderBuffer] | None]


def _copy_sampler(value: object) -> ImageSamplerDescriptor:
    if not isinstance(value, ImageSamplerDescriptor):
        raise TypeError("Material sampler fields require ImageSamplerDescriptor")
    return cast(
        ImageSamplerDescriptor,
        _pybevy.pbr.StorageShaderMaterial._copy_sampler_descriptor(value),
    )


@dataclass(frozen=True, init=False)
class StorageBinding(Generic[_Element]):
    """Declare a read-only array of a WGSL host-shareable element type.

    ``element`` accepts float, int, U32, Vec2/3/4, shader_type classes or WGSL text.
    Manual WGSL structs belong in ``declarations``. In ``StorageBinding[T]`` fields, T selects the
    element type; an omitted ``element`` otherwise defaults to Vec4.
    Upload bytes with the reported storage stride;
    this metadata does not serialize Python objects or add padding to data.
    """

    __module__ = "pybevy.material"
    _element: type | str
    _wgsl_type: str
    _declarations: str
    _stride: int
    _alignment: int
    _slot: int | None = dataclass_field(
        default=None, init=False, repr=False, compare=False
    )
    _explicit_element: bool = dataclass_field(
        default=False, init=False, repr=False, compare=False
    )

    def __init__(
        self,
        *,
        element: type | str | None = None,
        declarations: str = "",
        default: None = None,
    ) -> None:
        if default is not None:
            raise TypeError("StorageBinding default must be None")
        object.__setattr__(self, "_slot", None)
        object.__setattr__(self, "_explicit_element", element is not None)
        if element is None:
            element = Vec4
        if element == U32:
            element = "u32"
        types = {
            float: "f32",
            int: "i32",
            Vec2: "vec2<f32>",
            Vec3: "vec3<f32>",
            Vec4: "vec4<f32>",
        }
        wgsl_type: str | None
        if isinstance(element, type) and element not in types:
            try:
                layout = shader_type_layout(element)
            except TypeError as error:
                raise TypeError(
                    "StorageBinding element must be float, int, U32, Vec2/3/4, a @shader_type type or a WGSL type string"
                ) from error
            if declarations:
                raise TypeError(
                    "Shader struct bindings generate their own declarations"
                )
            wgsl_type = layout.wgsl_type
            declarations = layout.declarations
            element = layout.wgsl_type
        else:
            wgsl_type = element if isinstance(element, str) else types.get(element)
        if wgsl_type is None:
            raise TypeError(
                "StorageBinding element must be float, int, U32, Vec2/3/4, or a WGSL type string"
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
            object.__setattr__(self, f"_{name}", value)

    @classmethod
    def __class_getitem__(cls, element: object) -> GenericAlias:
        if isinstance(element, tuple):
            if len(element) != 1:
                raise TypeError("StorageBinding requires exactly one element type")
            element = element[0]
        # typing.Generic's subscription cache would retain reloaded record classes.
        return GenericAlias(cls, element)

    @property
    def element(self) -> type | str:
        return self._element

    @property
    def wgsl_type(self) -> str:
        return self._wgsl_type

    @property
    def declarations(self) -> str:
        return self._declarations

    @property
    def stride(self) -> int:
        return self._stride

    @property
    def alignment(self) -> int:
        return self._alignment

    @overload
    def __get__(
        self, instance: None, owner: type[object]
    ) -> StorageBinding[_Element]: ...
    @overload
    def __get__(
        self, instance: object, owner: type[object]
    ) -> Handle[ShaderBuffer] | None: ...

    def __get__(
        self, instance: object | None, owner: type[object]
    ) -> StorageBinding[_Element] | Handle[ShaderBuffer] | None:
        if instance is None:
            return self
        slot = self._bound_slot()
        material = cast(_BindingInstance, instance)
        if material._shader_mat is not None:
            return material._shader_mat.get_storage_buffer_at(slot)
        return material._storage_buffers[slot]

    def __set__(self, instance: object, value: Handle[ShaderBuffer] | None) -> None:
        slot = self._bound_slot()
        material = cast(_BindingInstance, instance)
        if material._shader_mat is not None:
            material._shader_mat.set_storage_buffer_at(slot, value)
        else:
            # The same validator runs before constructor values are accepted.
            _pybevy.pbr.StorageShaderMaterial._validate_storage_handle(value)
            material._storage_buffers[slot] = value

    def _bound_slot(self) -> int:
        if self._slot is None:
            raise TypeError("StorageBinding fields require @material")
        return self._slot


class _StorageLayout(Protocol):
    @property
    def wgsl_type(self) -> str: ...
    @property
    def declarations(self) -> str: ...
    @property
    def stride(self) -> int: ...


class _StorageField(NamedTuple):
    name: str
    binding: int
    layout: _StorageLayout


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
                        f"StorageBinding field '{name}' needs Annotated[Handle[ShaderBuffer] | None, StorageBinding()]"
                    )
                _validate_binding_identifier(name, "storage field")
                if name == "material":
                    raise TypeError(
                        "StorageBinding field 'material' conflicts with the generated uniform binding"
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
class TextureBinding:
    """Declare an Image binding's dimension, sample type and sampler_type contract.

    Configure actual filtering/addressing and view dimensions on the Image.
    None uses Bevy's white fallback for float textures; depth/integer stay pending.
    """

    __module__ = "pybevy.material"
    dimension: str
    sample_type: str
    filterable: bool
    sampler_type: str | None
    wgsl_type: str
    _wgsl_sampler_type: str
    _slot: int | None = dataclass_field(
        default=None, init=False, repr=False, compare=False
    )

    def __init__(
        self,
        *,
        dimension: str = "2d",
        sample_type: str = "float",
        filterable: bool = True,
        sampler_type: str | None = "filtering",
    ) -> None:
        object.__setattr__(self, "_slot", None)
        if sampler_type not in (None, "filtering", "non_filtering", "comparison"):
            raise ValueError(
                "TextureBinding sampler_type must be filtering, non_filtering, comparison, or None"
            )
        wgsl_type, wgsl_sampler_type = _pybevy.pbr.StorageShaderMaterial._texture_layout(
            dimension, sample_type, filterable, sampler_type
        )
        for name, value in (
            ("dimension", dimension),
            ("sample_type", sample_type),
            ("filterable", filterable),
            ("sampler_type", sampler_type),
            ("wgsl_type", wgsl_type),
            ("_wgsl_sampler_type", wgsl_sampler_type),
        ):
            object.__setattr__(self, name, value)

    @overload
    def __get__(self, instance: None, owner: type[object]) -> TextureBinding: ...
    @overload
    def __get__(
        self, instance: object, owner: type[object]
    ) -> Handle[Image] | None: ...

    def __get__(
        self, instance: object | None, owner: type[object]
    ) -> TextureBinding | Handle[Image] | None:
        if instance is None:
            return self
        slot = self._bound_slot()
        material = cast(_BindingInstance, instance)
        if material._shader_mat is not None:
            return material._shader_mat.get_texture(slot)
        return material._textures[slot]

    def __set__(self, instance: object, value: Handle[Image] | None) -> None:
        slot = self._bound_slot()
        material = cast(_BindingInstance, instance)
        if material._shader_mat is not None:
            if value is None:
                material._shader_mat.clear_texture(slot)
            else:
                material._shader_mat.set_texture(slot, value)
        else:
            _pybevy.pbr.StorageShaderMaterial._validate_texture_handle(value)
            material._textures[slot] = value

    def _bound_slot(self) -> int:
        if self._slot is None:
            raise TypeError("TextureBinding fields require @material")
        return self._slot


def _partition_texture_fields(
    hints: Mapping[str, object],
) -> tuple[dict[str, object], dict[str, TextureBinding]]:
    ordinary: dict[str, object] = {}
    textures: dict[str, TextureBinding] = {}
    for name, annotation in hints.items():
        if get_origin(annotation) is Annotated and not name.startswith("_"):
            field_type, *metadata = get_args(annotation)
            markers = [value for value in metadata if isinstance(value, TextureBinding)]
            if markers:
                if (
                    len(markers) != 1
                    or any(isinstance(value, StorageBinding) for value in metadata)
                    or not _is_texture_annotation(field_type)
                ):
                    raise TypeError(
                        f"TextureBinding field '{name}' needs Annotated[Handle[Image] | None, TextureBinding()]"
                    )
                textures[name] = markers[0]
                annotation = field_type
        ordinary[name] = annotation
    return ordinary, textures


@dataclass(frozen=True, init=False, slots=True)
class SamplerBinding:
    """Declare an independent ImageSamplerDescriptor material field."""

    __module__ = "pybevy.material"
    sampler_type: str
    _default: ImageSamplerDescriptor = dataclass_field(
        init=False, repr=False, compare=False
    )
    _slot: int | None = dataclass_field(
        default=None, init=False, repr=False, compare=False
    )

    def __init__(
        self,
        sampler_type: str = "filtering",
        *,
        default: ImageSamplerDescriptor = _DEFAULT_SAMPLER,
    ) -> None:
        if sampler_type not in ("filtering", "non_filtering", "comparison"):
            raise ValueError(
                "SamplerBinding sampler_type must be filtering, non_filtering, or comparison"
            )

        object.__setattr__(self, "sampler_type", sampler_type)
        object.__setattr__(self, "_slot", None)
        object.__setattr__(self, "_default", _copy_sampler(default))

    @property
    def wgsl_type(self) -> str:
        return "sampler_comparison" if self.sampler_type == "comparison" else "sampler"

    @overload
    def __get__(self, instance: None, owner: type[object]) -> SamplerBinding: ...
    @overload
    def __get__(
        self, instance: object, owner: type[object]
    ) -> ImageSamplerDescriptor: ...

    def __get__(
        self, instance: object | None, owner: type[object]
    ) -> SamplerBinding | ImageSamplerDescriptor:
        if instance is None:
            return self
        slot = self._bound_slot()
        material = cast(_BindingInstance, instance)
        if material._shader_mat is not None:
            return material._shader_mat.get_sampler(slot)
        return material._samplers[slot]

    def __set__(self, instance: object, value: ImageSamplerDescriptor) -> None:
        slot = self._bound_slot()
        material = cast(_BindingInstance, instance)
        if material._shader_mat is not None:
            material._shader_mat.set_sampler(slot, value)
        else:
            material._samplers[slot] = _copy_sampler(value)

    def _bound_slot(self) -> int:
        if self._slot is None:
            raise TypeError("SamplerBinding fields require @material")
        return self._slot


class _SamplerLayout(Protocol):
    @property
    def sampler_type(self) -> str: ...
    @property
    def wgsl_type(self) -> str: ...
    @property
    def _default(self) -> ImageSamplerDescriptor: ...


class _SamplerField(NamedTuple):
    name: str
    layout: _SamplerLayout


def _partition_sampler_fields(
    hints: Mapping[str, object],
) -> tuple[dict[str, object], tuple[_SamplerField, ...]]:
    ordinary: dict[str, object] = {}
    samplers: list[_SamplerField] = []
    for name, annotation in hints.items():
        if get_origin(annotation) is Annotated and not name.startswith("_"):
            field_type, *metadata = get_args(annotation)
            markers = [value for value in metadata if isinstance(value, SamplerBinding)]
            if markers:
                if (
                    len(markers) != 1
                    or field_type is not ImageSamplerDescriptor
                    or any(
                        isinstance(value, TextureBinding | StorageBinding)
                        for value in metadata
                    )
                ):
                    raise TypeError(
                        f"SamplerBinding field '{name}' needs Annotated[ImageSamplerDescriptor, SamplerBinding()]"
                    )
                _validate_binding_identifier(name, "sampler field")
                samplers.append(_SamplerField(name, markers[0]))
                continue
        ordinary[name] = annotation
    return ordinary, tuple(samplers)
