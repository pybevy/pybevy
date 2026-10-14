from collections.abc import Callable
from typing import ClassVar, Generic, Literal, TypeVar, dataclass_transform, overload

from pybevy.assets import Handle
from pybevy.math import Vec4
from pybevy.pbr import Material
from pybevy.render import ShaderBuffer

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

_Element = TypeVar("_Element")

class Storage(Generic[_Element]):
    def __init__(self, *, default: None) -> None: ...
    @overload
    def __get__(self, instance: None, owner: type[object]) -> Storage[_Element]: ...
    @overload
    def __get__(self, instance: object, owner: type[object]) -> Handle[ShaderBuffer] | None: ...
    def __set__(self, instance: object, value: Handle[ShaderBuffer] | None) -> None: ...

@dataclass_transform(kw_only_default=True, eq_default=False, field_specifiers=(Storage,))
def material(
    fragment_shader: str | None = None,
    vertex_shader: str | None = None,
    *,
    prepass_vertex_shader: str | None = None,
    prepass_fragment_shader: str | None = None,
    deferred_vertex_shader: str | None = None,
    deferred_fragment_shader: str | None = None,
    alpha_mode: AlphaMode | None = None,
    double_sided: bool | None = None,
    cull_mode: object = ...,
    unlit: bool | None = None,
    depth_bias: float | None = None,
) -> Callable[[type[MaterialT]], type[MaterialT]]:
    """Decorator to define a custom shader material.

    Shader paths must name WGSL assets. Prepass hooks cover depth, normals,
    motion vectors and shadows; deferred hooks write the G-buffer. Omitted
    paths keep native defaults. Forward stages are not reused automatically.

    A uniform field name must not end in a digit: upstream WGSL composition
    escapes such a struct member (``tint2`` becomes ``tint2_``) without
    patching the references, so the decorator rejects it. End the name in a
    letter or an underscore instead.
    """

class StorageBuffer:
    """Immutable Annotated metadata for a Handle[ShaderBuffer] | None field.

    One read-only array<vec4<f32>> at binding 109 per material. Use element=Vec4.
    """
    def __init__(self, *, element: type[Vec4] = Vec4) -> None: ...
    @property
    def element(self) -> type[Vec4]: ...
