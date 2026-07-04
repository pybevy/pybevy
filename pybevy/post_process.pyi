from typing import ClassVar

from pybevy.color import Color
from pybevy.ecs import Batchable, Component
from pybevy.math import Vec2

try:
    import numpy as np
except ImportError:
    pass

class BloomCompositeMode:
    """Bloom composite mode controlling how bloom is blended with the image."""

    EnergyConserving: BloomCompositeMode
    Additive: BloomCompositeMode

class BloomPrefilter:
    """Threshold filter for extracting bright regions before bloom."""

    threshold: float
    threshold_softness: float

    def __init__(self, threshold: float = 0.0, threshold_softness: float = 0.0) -> None: ...

class Bloom(Component):
    """Post-processing bloom effect for glowing lights and bright surfaces."""

    NATURAL: ClassVar[Bloom]
    ANAMORPHIC: ClassVar[Bloom]
    OLD_SCHOOL: ClassVar[Bloom]
    SCREEN_BLUR: ClassVar[Bloom]

    def __init__(
        self,
        intensity: float = 0.15,
        low_frequency_boost: float = 0.7,
        low_frequency_boost_curvature: float = 0.95,
        high_pass_frequency: float = 1.0,
        prefilter: BloomPrefilter | None = None,
        composite_mode: BloomCompositeMode = ...,
        max_mip_dimension: int = 512,
        scale: Vec2 | None = None,
    ) -> None: ...

    @property
    def intensity(self) -> float: ...
    @intensity.setter
    def intensity(self, value: float) -> None: ...

    @property
    def low_frequency_boost(self) -> float: ...
    @low_frequency_boost.setter
    def low_frequency_boost(self, value: float) -> None: ...

    @property
    def low_frequency_boost_curvature(self) -> float: ...
    @low_frequency_boost_curvature.setter
    def low_frequency_boost_curvature(self, value: float) -> None: ...

    @property
    def high_pass_frequency(self) -> float: ...
    @high_pass_frequency.setter
    def high_pass_frequency(self, value: float) -> None: ...

    @property
    def prefilter(self) -> BloomPrefilter: ...
    @prefilter.setter
    def prefilter(self, value: BloomPrefilter) -> None: ...

    @property
    def composite_mode(self) -> BloomCompositeMode: ...
    @composite_mode.setter
    def composite_mode(self, value: BloomCompositeMode) -> None: ...

    @property
    def max_mip_dimension(self) -> int: ...
    @max_mip_dimension.setter
    def max_mip_dimension(self, value: int) -> None: ...

    @property
    def scale(self) -> Vec2: ...
    @scale.setter
    def scale(self, value: Vec2) -> None: ...

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        intensity: np.ndarray | None = None,
        low_frequency_boost: np.ndarray | None = None,
        low_frequency_boost_curvature: np.ndarray | None = None,
        high_pass_frequency: np.ndarray | None = None,
        max_mip_dimension: np.ndarray | None = None,
    ) -> Batchable: ...

class Vignette(Component):
    """Post-processing vignette effect that darkens the periphery of the frame."""

    def __init__(
        self,
        intensity: float = 1.0,
        radius: float = 0.75,
        smoothness: float = 5.0,
        roundness: float = 1.0,
        center: Vec2 | None = None,
        edge_compensation: float = 1.0,
        color: Color | None = None,
    ) -> None: ...

    @property
    def intensity(self) -> float: ...
    @intensity.setter
    def intensity(self, value: float) -> None: ...

    @property
    def radius(self) -> float: ...
    @radius.setter
    def radius(self, value: float) -> None: ...

    @property
    def smoothness(self) -> float: ...
    @smoothness.setter
    def smoothness(self, value: float) -> None: ...

    @property
    def roundness(self) -> float: ...
    @roundness.setter
    def roundness(self, value: float) -> None: ...

    @property
    def center(self) -> Vec2: ...
    @center.setter
    def center(self, value: Vec2) -> None: ...

    @property
    def edge_compensation(self) -> float: ...
    @edge_compensation.setter
    def edge_compensation(self, value: float) -> None: ...

    @property
    def color(self) -> Color: ...
    @color.setter
    def color(self, value: Color) -> None: ...

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        intensity: np.ndarray | None = None,
        radius: np.ndarray | None = None,
        smoothness: np.ndarray | None = None,
        roundness: np.ndarray | None = None,
        edge_compensation: np.ndarray | None = None,
    ) -> Batchable: ...

class LensDistortion(Component):
    """Post-processing lens distortion effect (barrel/pincushion warping)."""

    def __init__(
        self,
        intensity: float = 0.5,
        scale: float = 1.0,
        multiplier: Vec2 | None = None,
        center: Vec2 | None = None,
        edge_curvature: float = 0.0,
    ) -> None: ...

    @property
    def intensity(self) -> float: ...
    @intensity.setter
    def intensity(self, value: float) -> None: ...

    @property
    def scale(self) -> float: ...
    @scale.setter
    def scale(self, value: float) -> None: ...

    @property
    def multiplier(self) -> Vec2: ...
    @multiplier.setter
    def multiplier(self, value: Vec2) -> None: ...

    @property
    def center(self) -> Vec2: ...
    @center.setter
    def center(self, value: Vec2) -> None: ...

    @property
    def edge_curvature(self) -> float: ...
    @edge_curvature.setter
    def edge_curvature(self, value: float) -> None: ...

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        intensity: np.ndarray | None = None,
        scale: np.ndarray | None = None,
        edge_curvature: np.ndarray | None = None,
    ) -> Batchable: ...
