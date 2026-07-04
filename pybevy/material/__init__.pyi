from collections.abc import Callable
from typing import ClassVar

class AlphaMode:
    """Alpha blending mode for materials.

    Can be used either as callable constructors (``AlphaMode.Blend()``) or
    pre-built class attributes (``AlphaMode.BLEND``).
    """

    OPAQUE: AlphaMode
    BLEND: AlphaMode
    PREMULTIPLIED: AlphaMode
    ADD: AlphaMode
    MULTIPLY: AlphaMode
    ALPHA_TO_COVERAGE: AlphaMode

    @staticmethod
    def Opaque() -> AlphaMode: ...
    @staticmethod
    def Mask(alpha: float) -> AlphaMode: ...
    @staticmethod
    def Blend() -> AlphaMode: ...
    @staticmethod
    def Premultiplied() -> AlphaMode: ...
    @staticmethod
    def Add() -> AlphaMode: ...
    @staticmethod
    def Multiply() -> AlphaMode: ...
    @staticmethod
    def AlphaToCoverage() -> AlphaMode: ...

class OpaqueRendererMethod:
    """Opaque rendering method selection."""

    Forward: ClassVar[OpaqueRendererMethod]
    Deferred: ClassVar[OpaqueRendererMethod]
    Auto: ClassVar[OpaqueRendererMethod]
    FORWARD: ClassVar[OpaqueRendererMethod]
    DEFERRED: ClassVar[OpaqueRendererMethod]
    AUTO: ClassVar[OpaqueRendererMethod]

def material(
    fragment_shader: str | None = None,
    vertex_shader: str | None = None,
    *,
    alpha_mode: AlphaMode | None = None,
    double_sided: bool | None = None,
    cull_mode: object = ...,
    unlit: bool | None = None,
    depth_bias: float | None = None,
) -> Callable[[type], type]:
    """Decorator to define a custom shader material."""
