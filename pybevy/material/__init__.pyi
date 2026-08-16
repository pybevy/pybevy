from collections.abc import Callable
from typing import ClassVar, Literal, TypeVar

from pybevy.pbr import Material

MaterialT = TypeVar("MaterialT", bound=Material)

class AlphaMode:
    """Alpha blending mode for materials."""

    class Opaque(AlphaMode):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Mask(AlphaMode):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
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

    Forward: ClassVar[OpaqueRendererMethod]
    Deferred: ClassVar[OpaqueRendererMethod]
    Auto: ClassVar[OpaqueRendererMethod]

def material(
    fragment_shader: str | None = None,
    vertex_shader: str | None = None,
    *,
    alpha_mode: AlphaMode | None = None,
    double_sided: bool | None = None,
    cull_mode: object = ...,
    unlit: bool | None = None,
    depth_bias: float | None = None,
) -> Callable[[type[MaterialT]], type[MaterialT]]:
    """Decorator to define a custom shader material."""
