"""Replace constant-like factories with descriptors for Pythonic attribute access.

Converts ``Color.WHITE()`` (staticmethod call) into ``Color.WHITE`` (attribute access)
by wrapping each factory in a descriptor that calls it on every ``__get__``, returning
a fresh instance each time. This prevents the singleton mutation bug that ``#[classattr]``
had while supporting both ``UPPER_CASE`` associated constants and PascalCase enum variants.
"""

# ruff: noqa: PLC0415
from __future__ import annotations


class _ConstantFactory:
    """Descriptor that invokes a factory callable on each attribute access."""

    __slots__ = ("_factory",)

    def __init__(self, factory: object) -> None:
        self._factory = factory

    def __get__(self, obj: object, cls: type | None = None) -> object:
        return self._factory()  # type: ignore[operator]

    def __set__(self, obj: object, value: object) -> None:
        raise AttributeError("read-only constant")


def _apply() -> None:
    """Wrap registered static factories with ``_ConstantFactory`` descriptors."""
    from .assets import RenderAssetUsages
    from .audio import PlaybackSettings, Volume
    from .camera import Exposure, Visibility
    from .color import Color, Hwba, Laba, LinearRgba, Oklaba, Srgba, Xyza
    from .core_pipeline import Tonemapping
    from .light import ShadowFilteringMethod, SunDisk
    from .math import (
        Affine2,
        Affine3A,
        BVec2,
        Dir2,
        Dir3,
        Interval,
        IRect,
        Isometry2d,
        Isometry3d,
        IVec2,
        Mat2,
        Mat3,
        Mat3A,
        Mat4,
        Quat,
        Rect,
        Rot2,
        URect,
        UVec2,
        UVec3,
        Vec2,
        Vec3,
        Vec3A,
        Vec4,
    )
    from .post_process import Bloom
    from .render import Backends, TextureUsages
    from .sprite import Anchor
    from .text import TextBackgroundColor, TextBounds, TextColor
    from .transform import GlobalTransform, Transform
    from .ui import (
        BorderRadius,
        FocusPolicy,
        Interaction,
        Overflow,
        UiPosition,
        UiRect,
        UiTransform,
        Val2,
    )

    _registry: list[tuple[type, list[str]]] = [
        # color
        (Color, ["WHITE", "BLACK", "NONE"]),
        (LinearRgba, ["BLACK", "WHITE", "NONE", "NAN"]),
        (Srgba, ["BLACK", "WHITE", "NONE"]),
        (Hwba, ["BLACK", "WHITE"]),
        (Laba, ["BLACK", "WHITE"]),
        (Oklaba, ["BLACK", "WHITE"]),
        (Xyza, ["BLACK", "WHITE"]),
        # camera
        (Visibility, ["Inherited", "Visible", "Hidden"]),
        (Bloom, ["NATURAL", "ANAMORPHIC", "OLD_SCHOOL", "SCREEN_BLUR"]),
        (Exposure, ["SUNLIGHT", "OVERCAST", "INDOOR", "BLENDER"]),
        # core_pipeline
        (
            Tonemapping,
            [
                "None_",
                "Reinhard",
                "ReinhardLuminance",
                "AcesFitted",
                "AgX",
                "SomewhatBoringDisplayTransform",
                "TonyMcMapface",
                "BlenderFilmic",
                "KhronosPbrNeutral",
            ],
        ),
        # light
        (ShadowFilteringMethod, ["Hardware2x2", "Gaussian", "Temporal"]),
        # assets
        (RenderAssetUsages, ["MAIN_WORLD", "RENDER_WORLD"]),
        (
            TextureUsages,
            [
                "COPY_SRC",
                "COPY_DST",
                "TEXTURE_BINDING",
                "STORAGE_BINDING",
                "RENDER_ATTACHMENT",
                "STORAGE_ATOMIC",
                "TRANSIENT",
            ],
        ),
        (
            Backends,
            [
                "NOOP",
                "VULKAN",
                "GL",
                "METAL",
                "DX12",
                "BROWSER_WEBGPU",
                "PRIMARY",
                "SECONDARY",
            ],
        ),
        # light
        (SunDisk, ["EARTH", "OFF"]),
        # math
        (
            Vec2,
            [
                "ZERO",
                "ONE",
                "NEG_ONE",
                "MIN",
                "MAX",
                "INFINITY",
                "NEG_INFINITY",
                "NAN",
                "X",
                "Y",
                "NEG_X",
                "NEG_Y",
                "AXES",
            ],
        ),
        (
            Vec3,
            [
                "ZERO",
                "ONE",
                "NEG_ONE",
                "MIN",
                "MAX",
                "NAN",
                "INFINITY",
                "NEG_INFINITY",
                "X",
                "Y",
                "Z",
                "NEG_X",
                "NEG_Y",
                "NEG_Z",
                "AXES",
            ],
        ),
        (
            Vec3A,
            [
                "ZERO",
                "ONE",
                "NEG_ONE",
                "MIN",
                "MAX",
                "NAN",
                "INFINITY",
                "NEG_INFINITY",
                "X",
                "Y",
                "Z",
                "NEG_X",
                "NEG_Y",
                "NEG_Z",
                "AXES",
            ],
        ),
        (
            Vec4,
            [
                "ZERO",
                "ONE",
                "NEG_ONE",
                "MIN",
                "MAX",
                "NAN",
                "INFINITY",
                "NEG_INFINITY",
                "X",
                "Y",
                "Z",
                "W",
                "NEG_X",
                "NEG_Y",
                "NEG_Z",
                "NEG_W",
                "AXES",
            ],
        ),
        (Isometry2d, ["IDENTITY"]),
        (Isometry3d, ["IDENTITY"]),
        (
            IVec2,
            [
                "ZERO",
                "ONE",
                "NEG_ONE",
                "X",
                "Y",
                "NEG_X",
                "NEG_Y",
                "MIN",
                "MAX",
                "AXES",
            ],
        ),
        (BVec2, ["FALSE", "TRUE"]),
        (UVec2, ["ZERO", "ONE", "X", "Y", "MIN", "MAX", "AXES"]),
        (UVec3, ["ZERO", "ONE", "X", "Y", "Z", "MIN", "MAX", "AXES"]),
        (Quat, ["IDENTITY", "NAN"]),
        (Affine2, ["IDENTITY", "ZERO", "NAN"]),
        (Affine3A, ["IDENTITY", "ZERO", "NAN"]),
        (Mat2, ["IDENTITY", "ZERO", "NAN"]),
        (Mat3, ["IDENTITY", "ZERO", "NAN"]),
        (Mat3A, ["IDENTITY", "ZERO", "NAN"]),
        (Mat4, ["IDENTITY", "ZERO", "NAN"]),
        (
            Dir2,
            [
                "X",
                "Y",
                "NEG_X",
                "NEG_Y",
                "NORTH",
                "SOUTH",
                "EAST",
                "WEST",
                "NORTH_EAST",
                "NORTH_WEST",
                "SOUTH_EAST",
                "SOUTH_WEST",
            ],
        ),
        (Dir3, ["X", "Y", "Z", "NEG_X", "NEG_Y", "NEG_Z"]),
        (
            Rot2,
            [
                "IDENTITY",
                "PI",
                "FRAC_PI_2",
                "FRAC_PI_3",
                "FRAC_PI_4",
                "FRAC_PI_6",
                "FRAC_PI_8",
            ],
        ),
        (Rect, ["EMPTY"]),
        (URect, ["EMPTY"]),
        (IRect, ["EMPTY"]),
        (Interval, ["UNIT", "EVERYWHERE"]),
        # sprite
        (
            Anchor,
            [
                "CENTER",
                "BOTTOM_LEFT",
                "BOTTOM_CENTER",
                "BOTTOM_RIGHT",
                "CENTER_LEFT",
                "CENTER_RIGHT",
                "TOP_LEFT",
                "TOP_CENTER",
                "TOP_RIGHT",
            ],
        ),
        # text
        (TextColor, ["BLACK", "WHITE"]),
        (TextBackgroundColor, ["BLACK", "WHITE"]),
        (TextBounds, ["UNBOUNDED"]),
        # transform
        (Transform, ["IDENTITY"]),
        (GlobalTransform, ["IDENTITY"]),
        # ui
        (FocusPolicy, ["Block", "Pass"]),
        (Interaction, ["None_", "Hovered", "Pressed"]),
        (Overflow, ["DEFAULT"]),
        (BorderRadius, ["ZERO", "MAX"]),
        (UiTransform, ["IDENTITY"]),
        (UiPosition, ["CENTER", "TOP", "BOTTOM", "LEFT", "RIGHT", "TOP_LEFT", "TOP_RIGHT", "BOTTOM_LEFT", "BOTTOM_RIGHT"]),
        (UiRect, ["ZERO", "AUTO", "DEFAULT"]),
        (Val2, ["ZERO"]),
        # audio
        (PlaybackSettings, ["ONCE", "LOOP", "DESPAWN", "REMOVE"]),
        (Volume, ["SILENT"]),
    ]

    for cls, names in _registry:
        for name in names:
            factory = getattr(cls, name)
            setattr(cls, name, _ConstantFactory(factory))
