from typing import ClassVar

from pybevy.camera import PhysicalCameraParameters
from pybevy.color import Color
from pybevy.ecs import Batchable, Component
from pybevy.math import Vec2

try:
    import numpy as np
except ImportError:
    pass

class BloomCompositeMode:
    """Bloom composite mode controlling how bloom is blended with the image."""

    def __copy__(self) -> BloomCompositeMode: ...
    def __deepcopy__(self, memo: dict[int, object]) -> BloomCompositeMode: ...


    EnergyConserving: BloomCompositeMode
    Additive: BloomCompositeMode

    def __hash__(self) -> int: ...

class BloomPrefilter:
    """Threshold filter for extracting bright regions before bloom."""

    threshold: float
    threshold_softness: float

    def __init__(self, *, threshold: float = 0.0, threshold_softness: float = 0.0) -> None: ...

class Bloom(Component):
    """Post-processing bloom effect for glowing lights and bright surfaces."""

    NATURAL: ClassVar[Bloom]
    ANAMORPHIC: ClassVar[Bloom]
    OLD_SCHOOL: ClassVar[Bloom]
    SCREEN_BLUR: ClassVar[Bloom]

    def __init__(
        self,
        *,
        intensity: float = 0.15,
        low_frequency_boost: float = 0.7,
        low_frequency_boost_curvature: float = 0.95,
        high_pass_frequency: float = 1.0,
        prefilter: BloomPrefilter = BloomPrefilter(),
        composite_mode: BloomCompositeMode = ...,
        max_mip_dimension: int = 512,
        scale: Vec2 = Vec2.ONE
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
    def high_pass_frequency(self) -> float:
        """Above 0.0, up to 1.0; 0.0 raises ValueError."""
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
    def max_mip_dimension(self) -> int:
        """Largest bloom mip dimension, in pixels; 8 to 16384."""

    @max_mip_dimension.setter
    def max_mip_dimension(self, value: int) -> None:
        """Raise ValueError outside 8 to 16384."""

    @property
    def scale(self) -> Vec2: ...
    @scale.setter
    def scale(self, value: Vec2) -> None: ...

    @staticmethod
    def batch(  # type: ignore[override]
        *,
        intensity: np.typing.ArrayLike | None = None,
        low_frequency_boost: np.typing.ArrayLike | None = None,
        low_frequency_boost_curvature: np.typing.ArrayLike | None = None,
        high_pass_frequency: np.typing.ArrayLike | None = None,
        max_mip_dimension: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class Vignette(Component):
    """Post-processing vignette effect that darkens the periphery of the frame."""

    def __init__(
        self,
        *,
        intensity: float = 1.0,
        radius: float = 0.75,
        smoothness: float = 5.0,
        roundness: float = 1.0,
        center: Vec2 = Vec2(0.5, 0.5),
        edge_compensation: float = 1.0,
        color: Color = Color.BLACK
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
    def batch(  # type: ignore[override]
        *,
        intensity: np.typing.ArrayLike | None = None,
        radius: np.typing.ArrayLike | None = None,
        smoothness: np.typing.ArrayLike | None = None,
        roundness: np.typing.ArrayLike | None = None,
        edge_compensation: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class LensDistortion(Component):
    """Post-processing lens distortion effect (barrel/pincushion warping)."""

    def __init__(
        self,
        *,
        intensity: float = 0.5,
        scale: float = 1.0,
        multiplier: Vec2 = Vec2.ONE,
        center: Vec2 = Vec2(0.5, 0.5),
        edge_curvature: float = 0.0
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
    def batch(  # type: ignore[override]
        *,
        intensity: np.typing.ArrayLike | None = None,
        scale: np.typing.ArrayLike | None = None,
        edge_curvature: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class MotionBlur(Component):
    """Per-object, per-pixel camera motion blur.

    Add to a ``Camera3d`` entity. Spawning it inserts ``MotionVectorPrepass``.
    ``shutter_angle`` must be finite; values at or below ``0.0`` and
    ``samples=0`` disable the effect. ``samples`` is at most 1024: each pixel
    takes ``2 * samples + 1`` taps.
    """

    def __init__(self, *, shutter_angle: float = 0.5, samples: int = 1) -> None: ...

    @property
    def shutter_angle(self) -> float: ...
    @shutter_angle.setter
    def shutter_angle(self, value: float) -> None: ...

    @property
    def samples(self) -> int: ...
    @samples.setter
    def samples(self, value: int) -> None: ...

    @staticmethod
    def batch(  # type: ignore[override]
        *,
        shutter_angle: np.typing.ArrayLike | None = None,
        samples: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class DepthOfFieldMode:
    """How ``DepthOfField`` renders out-of-focus areas."""

    def __copy__(self) -> DepthOfFieldMode: ...
    def __deepcopy__(self, memo: dict[int, object]) -> DepthOfFieldMode: ...

    Bokeh: DepthOfFieldMode
    Gaussian: DepthOfFieldMode

class DepthOfField(Component):
    """Camera depth of field that blurs objects away from the focal distance.

    Add to a ``Camera3d`` with a perspective projection; other projections
    render without the effect. It reads the main depth texture and supports
    every MSAA setting, but WebGL2 builds disable it. Float fields must be
    finite, except ``max_depth``, which also accepts positive infinity.
    """

    def __init__(
        self,
        *,
        mode: DepthOfFieldMode = DepthOfFieldMode.Gaussian,
        focal_distance: float = 10.0,
        sensor_height: float = 0.01866,
        aperture_f_stops: float = 1.0,
        max_circle_of_confusion_diameter: float = 64.0,
        max_depth: float = float("inf"),
    ) -> None: ...

    @staticmethod
    def from_physical_camera(camera: PhysicalCameraParameters) -> DepthOfField:
        """Copy ``sensor_height`` and ``aperture_f_stops``; other fields use defaults."""

    @property
    def mode(self) -> DepthOfFieldMode: ...
    @mode.setter
    def mode(self, value: DepthOfFieldMode) -> None: ...

    @property
    def focal_distance(self) -> float:
        """Distance in meters to the plane in focus."""

    @focal_distance.setter
    def focal_distance(self, value: float) -> None: ...

    @property
    def sensor_height(self) -> float:
        """Image sensor height in meters; with the FOV it sets the focal length."""

    @sensor_height.setter
    def sensor_height(self, value: float) -> None: ...

    @property
    def aperture_f_stops(self) -> float:
        """F-number; lower values blur out-of-focus objects more."""

    @aperture_f_stops.setter
    def aperture_f_stops(self, value: float) -> None: ...

    @property
    def max_circle_of_confusion_diameter(self) -> float:
        """Largest blur diameter in pixels, 0 to 16384.

        Larger values raise ValueError: the blur cost grows with the diameter,
        and a huge one stalls the GPU for seconds per frame.
        """

    @max_circle_of_confusion_diameter.setter
    def max_circle_of_confusion_diameter(self, value: float) -> None: ...

    @property
    def max_depth(self) -> float:
        """Farthest distance considered for blur; caps skybox and background blur."""

    @max_depth.setter
    def max_depth(self, value: float) -> None: ...

    @staticmethod
    def batch(  # type: ignore[override]
        *,
        focal_distance: np.typing.ArrayLike | None = None,
        sensor_height: np.typing.ArrayLike | None = None,
        aperture_f_stops: np.typing.ArrayLike | None = None,
        max_circle_of_confusion_diameter: np.typing.ArrayLike | None = None,
        max_depth: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...
