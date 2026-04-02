"""Replace UPPERCASE staticmethod factories with descriptors for Pythonic constant access.

Converts ``Color.WHITE()`` (staticmethod call) into ``Color.WHITE`` (attribute access)
by wrapping each factory in a descriptor that calls it on every ``__get__``, returning
a fresh instance each time.  This prevents the singleton mutation bug that ``#[classattr]``
had, while providing the idiomatic ``UPPER_CASE`` constant syntax Python users expect.
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
    """Wrap all UPPERCASE staticmethod factories with ``_ConstantFactory`` descriptors."""
    from .audio import PlaybackSettings, Volume
    from .camera import Exposure, Visibility
    from .color import Color, Laba, LinearRgba, Oklaba, Srgba, Xyza
    from .light import SunDisk
    from .math import Affine2, IVec2, Quat, Rot2, URect, UVec2, UVec3, Vec2, Vec3, Vec4
    from .post_process import Bloom
    from .sprite import Anchor
    from .text import TextBackgroundColor, TextBounds, TextColor
    from .transform import GlobalTransform, Transform
    from .ui import Overflow, UiRect, Val

    _registry: list[tuple[type, list[str]]] = [
        # color
        (Color, ["WHITE", "BLACK", "NONE"]),
        (LinearRgba, ["BLACK", "WHITE", "NONE", "NAN"]),
        (Srgba, ["BLACK", "WHITE", "NONE"]),
        (Laba, ["BLACK", "WHITE"]),
        (Oklaba, ["BLACK", "WHITE"]),
        (Xyza, ["BLACK", "WHITE"]),
        # camera
        (Visibility, ["INHERITED", "VISIBLE", "HIDDEN"]),
        (Bloom, ["NATURAL", "ANAMORPHIC"]),
        (Exposure, ["SUNLIGHT", "OVERCAST", "INDOOR", "BLENDER"]),
        # light
        (SunDisk, ["EARTH"]),
        # math
        (Vec2, [
            "ZERO", "ONE", "NEG_ONE", "MIN", "MAX",
            "INFINITY", "NEG_INFINITY", "NAN",
            "X", "Y", "NEG_X", "NEG_Y",
        ]),
        (Vec3, [
            "ZERO", "ONE", "NEG_ONE", "MIN", "MAX",
            "NAN", "INFINITY", "NEG_INFINITY",
            "X", "Y", "Z", "NEG_X", "NEG_Y", "NEG_Z",
        ]),
        (Vec4, [
            "ZERO", "ONE", "NEG_ONE", "MIN", "MAX",
            "NAN", "INFINITY", "NEG_INFINITY",
            "X", "Y", "Z", "W", "NEG_X", "NEG_Y", "NEG_Z", "NEG_W",
        ]),
        (IVec2, [
            "ZERO", "ONE", "NEG_ONE",
            "X", "Y", "NEG_X", "NEG_Y", "MIN", "MAX",
        ]),
        (UVec2, ["ZERO", "ONE", "X", "Y", "MIN", "MAX"]),
        (UVec3, ["ZERO", "ONE", "X", "Y", "Z", "MIN", "MAX"]),
        (Quat, ["IDENTITY", "NAN"]),
        (Affine2, ["IDENTITY", "ZERO", "NAN"]),
        (Rot2, [
            "IDENTITY", "PI",
            "FRAC_PI_2", "FRAC_PI_3", "FRAC_PI_4", "FRAC_PI_6", "FRAC_PI_8",
        ]),
        (URect, ["EMPTY"]),
        # sprite
        (Anchor, [
            "CENTER", "BOTTOM_LEFT", "BOTTOM_CENTER", "BOTTOM_RIGHT",
            "CENTER_LEFT", "CENTER_RIGHT",
            "TOP_LEFT", "TOP_CENTER", "TOP_RIGHT",
        ]),
        # text
        (TextColor, ["BLACK", "WHITE"]),
        (TextBackgroundColor, ["BLACK", "WHITE"]),
        (TextBounds, ["UNBOUNDED"]),
        # transform
        (Transform, ["IDENTITY"]),
        (GlobalTransform, ["IDENTITY"]),
        # ui
        (Overflow, ["DEFAULT"]),
        (UiRect, ["ZERO", "AUTO", "DEFAULT"]),
        (Val, ["ZERO"]),
        # audio
        (PlaybackSettings, ["ONCE", "LOOP", "DESPAWN", "REMOVE"]),
        (Volume, ["SILENT"]),
    ]

    for cls, names in _registry:
        for name in names:
            factory = getattr(cls, name)
            setattr(cls, name, _ConstantFactory(factory))
