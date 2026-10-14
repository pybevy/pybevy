from collections.abc import Callable, Iterable
from dataclasses import field
from typing import (
    Annotated,
    ClassVar,
    Generic,
    Literal,
    TypeVar,
    dataclass_transform,
    overload,
)

from pybevy.assets import Handle
from pybevy.image import Image, ImageSamplerDescriptor
from pybevy.math import Vec4
from pybevy.pbr import Material, StandardMaterial
from pybevy.render import ShaderBuffer

U32 = Annotated[int, "u32"]

MaterialT = TypeVar("MaterialT", bound=Material)

class AlphaMode:
    """Alpha blending mode for materials."""

    def __copy__(self) -> AlphaMode: ...
    def __deepcopy__(self, memo: dict[int, object]) -> AlphaMode: ...

    class Opaque(AlphaMode):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Mask(AlphaMode):
        __match_args__: ClassVar[tuple[Literal["value"]]] = ("value",)
        @property
        def value(self) -> float: ...
        def __init__(self, value: float) -> None: ...

    class Blend(AlphaMode):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Premultiplied(AlphaMode):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Add(AlphaMode):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Multiply(AlphaMode):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AlphaToCoverage(AlphaMode):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

class OpaqueRendererMethod:
    """Opaque rendering method selection."""

    def __copy__(self) -> OpaqueRendererMethod: ...
    def __deepcopy__(self, memo: dict[int, object]) -> OpaqueRendererMethod: ...

    Forward: ClassVar[OpaqueRendererMethod]
    Deferred: ClassVar[OpaqueRendererMethod]
    Auto: ClassVar[OpaqueRendererMethod]

_Element = TypeVar("_Element", default=Vec4)

@dataclass_transform(kw_only_default=True, eq_default=False)
def material(
    fragment_shader: str | None = None,
    vertex_shader: str | None = None,
    *,
    prepass_vertex_shader: str | None = None,
    prepass_fragment_shader: str | None = None,
    deferred_vertex_shader: str | None = None,
    deferred_fragment_shader: str | None = None,
    base: StandardMaterial | None = None,
) -> Callable[[type[MaterialT]], type[MaterialT]]:
    """Decorator to define a custom shader material.

    Always called: ``@material()`` with no options, or ``@material(...)``.

    Shader paths name ``.wesl`` assets, which import the generated
    ``pybevy::gen::<Class>`` module, or plain ``.wgsl`` assets without imports
    or conditional compilation. Prepass hooks cover depth, normals,
    motion vectors and shadows; deferred hooks write the G-buffer. Omitted
    paths keep native defaults. Forward stages are not reused automatically.
    """

class StorageBinding(Generic[_Element]):
    """Typed storage declaration or Annotated metadata; omitted element means Vec4."""
    def __init__(
        self, *, element: type | str | None = None, declarations: str = "", default: None = None
    ) -> None: ...
    @overload
    def __get__(self, instance: None, owner: type[object]) -> StorageBinding[_Element]: ...
    @overload
    def __get__(self, instance: object, owner: type[object]) -> Handle[ShaderBuffer] | None: ...
    def __set__(self, instance: object, value: Handle[ShaderBuffer] | None) -> None: ...

    @property
    def element(self) -> type | str: ...
    @property
    def wgsl_type(self) -> str: ...
    @property
    def declarations(self) -> str: ...
    @property
    def stride(self) -> int: ...
    @property
    def alignment(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class TextureBinding:
    """Image binding metadata or typed field declaration; instance values are handles."""
    def __init__(
        self,
        *,
        dimension: str = "2d",
        sample_type: str = "float",
        filterable: bool = True,
        sampler_type: str | None = "filtering",
    ) -> None: ...
    @overload
    def __get__(self, instance: None, owner: type[object]) -> TextureBinding: ...
    @overload
    def __get__(self, instance: object, owner: type[object]) -> Handle[Image] | None: ...
    def __set__(self, instance: object, value: Handle[Image] | None) -> None: ...
    @property
    def dimension(self) -> str: ...
    @property
    def sample_type(self) -> str: ...
    @property
    def filterable(self) -> bool: ...
    @property
    def sampler_type(self) -> str | None: ...
    @property
    def wgsl_type(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...


class SamplerBinding:
    """SamplerBinding metadata or typed field declaration; values are ImageSamplerDescriptor."""
    def __init__(self, sampler_type: str = "filtering", *, default: ImageSamplerDescriptor = ...) -> None: ...
    @overload
    def __get__(self, instance: None, owner: type[object]) -> SamplerBinding: ...
    @overload
    def __get__(self, instance: object, owner: type[object]) -> ImageSamplerDescriptor: ...
    def __set__(self, instance: object, value: ImageSamplerDescriptor) -> None: ...
    @property
    def sampler_type(self) -> str: ...
    @property
    def wgsl_type(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...


@dataclass_transform(field_specifiers=(field,))
def shader_type[Record](cls: type[Record]) -> type[Record]:
    """Generate a dataclass with native scalar/vector storage layout."""

def shader_type_layout(cls: type[object]) -> ShaderTypeLayout:
    """Return the immutable native WGSL storage layout of a decorated class."""

def pack_shader_data(values: Iterable[object], *, element: object = None) -> bytes:
    """Pack storage records/scalars/vectors; empty input requires element=."""

class ShaderTypeField:
    def __init__(self, name: str, wgsl_type: str, offset: int, size: int, alignment: int) -> None: ...
    @property
    def name(self) -> str: ...
    @property
    def wgsl_type(self) -> str: ...
    @property
    def offset(self) -> int: ...
    @property
    def size(self) -> int: ...
    @property
    def alignment(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class ShaderTypeLayout:
    def __init__(self, wgsl_type: str, declarations: str, size: int, stride: int, alignment: int, fields: tuple[ShaderTypeField, ...]) -> None: ...
    @property
    def wgsl_type(self) -> str: ...
    @property
    def declarations(self) -> str: ...
    @property
    def size(self) -> int: ...
    @property
    def stride(self) -> int: ...
    @property
    def alignment(self) -> int: ...
    @property
    def fields(self) -> tuple[ShaderTypeField, ...]: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
