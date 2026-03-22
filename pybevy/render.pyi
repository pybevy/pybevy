import numpy as np
import numpy.typing as npt

from pybevy.app import App, Plugin
from pybevy.assets import Asset
from pybevy.ecs import Component
from pybevy.math import Vec2
from pybevy.pbr import StandardMaterial as StandardMaterial
from pybevy.wgpu import Face as Face
from pybevy.wgpu import PowerPreference

PyFace = Face

class RenderPlugin(Plugin):
    """Configures the rendering pipeline.

    Examples:
        ```python
        from pybevy import DefaultPlugins, RenderPlugin, PowerPreference

        # Low power GPU
        app.add_plugins(
            DefaultPlugins().set(RenderPlugin(
                power_preference=PowerPreference.LowPower
            ))
        )

        # Synchronous pipeline compilation
        app.add_plugins(
            DefaultPlugins().set(RenderPlugin(
                synchronous_pipeline_compilation=True
            ))
        )
        ```
    """

    def __init__(
        self,
        power_preference: PowerPreference | None = None,
        synchronous_pipeline_compilation: bool | None = None,
    ) -> None:
        """Create a RenderPlugin with optional configuration.

        Args:
            power_preference: GPU adapter selection preference (None, LowPower, HighPerformance).
                If None, uses environment variable or defaults to HighPerformance.
            synchronous_pipeline_compilation: If True, disables async pipeline compilation.
                No effect on macOS, Wasm, or iOS.
        """

    def build(self, app: App) -> None: ...

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

class OpaqueRenderMethod:
    """Opaque rendering method selection (render module re-export)."""

    Forward: OpaqueRenderMethod
    Deferred: OpaqueRenderMethod
    Auto: OpaqueRenderMethod
    FORWARD: OpaqueRenderMethod
    DEFERRED: OpaqueRenderMethod
    AUTO: OpaqueRenderMethod

class ColorGradingSection:
    """Color grading values applied to a specific tonal range (shadows, midtones, or highlights).

    Supports borrowed access: mutations to sections obtained from ColorGrading.shadows etc.
    will persist back to the parent component.
    """

    def __init__(
        self,
        saturation: float = 1.0,
        contrast: float = 1.0,
        gamma: float = 1.0,
        gain: float = 1.0,
        lift: float = 0.0,
    ) -> None: ...

    @property
    def saturation(self) -> float: ...
    @saturation.setter
    def saturation(self, value: float) -> None: ...

    @property
    def contrast(self) -> float: ...
    @contrast.setter
    def contrast(self, value: float) -> None: ...

    @property
    def gamma(self) -> float: ...
    @gamma.setter
    def gamma(self, value: float) -> None: ...

    @property
    def gain(self) -> float: ...
    @gain.setter
    def gain(self, value: float) -> None: ...

    @property
    def lift(self) -> float: ...
    @lift.setter
    def lift(self, value: float) -> None: ...

    def __eq__(self, other: object) -> bool:
        """Check equality with another ColorGradingSection."""

class ColorGradingGlobal:
    """Global color grading values applied to the entire image.

    Supports borrowed access: mutations to global obtained from ColorGrading.global_
    will persist back to the parent component.
    """

    def __init__(
        self,
        exposure: float = 0.0,
        temperature: float = 0.0,
        tint: float = 0.0,
        hue: float = 0.0,
        post_saturation: float = 1.0,
        midtones_range: tuple[float, float] = (0.2, 0.7),
    ) -> None: ...

    @property
    def exposure(self) -> float: ...
    @exposure.setter
    def exposure(self, value: float) -> None: ...

    @property
    def temperature(self) -> float: ...
    @temperature.setter
    def temperature(self, value: float) -> None: ...

    @property
    def tint(self) -> float: ...
    @tint.setter
    def tint(self, value: float) -> None: ...

    @property
    def hue(self) -> float: ...
    @hue.setter
    def hue(self, value: float) -> None: ...

    @property
    def post_saturation(self) -> float: ...
    @post_saturation.setter
    def post_saturation(self, value: float) -> None: ...

    @property
    def midtones_range(self) -> tuple[float, float]: ...
    @midtones_range.setter
    def midtones_range(self, value: tuple[float, float]) -> None: ...

class Hdr(Component):
    """Marker component that enables HDR (High Dynamic Range) rendering for a camera."""

    def __init__(self) -> None: ...

class Msaa(Component):
    """Multi-sample anti-aliasing configuration.

    MSAA smooths jagged edges by sampling multiple points per pixel.
    Higher sample counts produce smoother edges at the cost of performance.

    Example:
        ```python
        from pybevy.render import Msaa

        # Disable MSAA for maximum performance
        commands.spawn((Camera3d(), Msaa.Off))

        # Use 4x MSAA (default)
        commands.spawn((Camera3d(), Msaa.Sample4))
        ```
    """

    Off: Msaa
    """No anti-aliasing (1 sample)."""

    Sample2: Msaa
    """2x multi-sample anti-aliasing."""

    Sample4: Msaa
    """4x multi-sample anti-aliasing (default)."""

    Sample8: Msaa
    """8x multi-sample anti-aliasing."""

    def samples(self) -> int:
        """Get the number of samples for this MSAA configuration."""

    @staticmethod
    def from_samples(samples: int) -> Msaa:
        """Create Msaa from the number of samples.

        Args:
            samples: Number of samples (1, 2, 4, or 8)

        Returns:
            Msaa configuration for the given sample count

        Raises:
            RuntimeError: If samples is not 1, 2, 4, or 8
        """

class TemporalJitter(Component):
    """A subpixel offset to jitter a perspective camera's frustum.

    Useful for temporal rendering techniques like TAA (Temporal Anti-Aliasing).
    The offset should be in the range [-0.5, 0.5].

    Examples:
        >>> jitter = TemporalJitter(offset=Vec2(0.25, -0.25))
        >>> commands.spawn(Camera3d(), Camera(), jitter)
    """

    def __init__(self, offset: Vec2 = Vec2.ZERO) -> None:
        """Create a new TemporalJitter component.

        Args:
            offset: Subpixel offset in range [-0.5, 0.5]. Default is zero (no jitter).
        """

    @property
    def offset(self) -> Vec2:
        """Get the current jitter offset."""

    @offset.setter
    def offset(self, value: Vec2) -> None:
        """Set the jitter offset."""

class MipBias(Component):
    """Camera component specifying a mip bias for texture sampling.

    Often used in conjunction with antialiasing post-process effects to reduce
    texture blurriness. A negative value (the default of -1.0) samples sharper
    mip levels.

    Examples:
        >>> mip_bias = MipBias(-1.0)
        >>> commands.spawn(Camera3d(), Camera(), mip_bias)
    """

    def __init__(self, value: float = -1.0) -> None:
        """Create a new MipBias component.

        Args:
            value: The mip bias value. Negative values (default -1.0) sample sharper textures.
        """

    @property
    def value(self) -> float:
        """Get the mip bias value."""

class OcclusionCulling(Component):
    """Enable GPU occlusion culling for a camera.

    Occlusion culling allows Bevy to avoid rendering objects that are fully
    behind other opaque or alpha-tested objects. This can significantly improve
    performance in scenes with high depth complexity.

    Note: Occlusion culling currently requires a DepthPrepass component on the
    camera. If no depth prepass is present, this component will be ignored.

    Examples:
        >>> commands.spawn(Camera3d(), Camera(), DepthPrepass(), OcclusionCulling())
    """

    def __init__(self) -> None:
        """Create an OcclusionCulling marker component."""

class NoAutomaticBatching(Component):
    """Marker component to disable automatic batching for a mesh entity.

    Most applications will not need this component, as automatic batching
    generally improves performance.

    Examples:
        >>> commands.spawn(Mesh3d(mesh_handle), MeshMaterial3d(mat_handle), NoAutomaticBatching())
    """

    def __init__(self) -> None:
        """Create a NoAutomaticBatching marker component."""

class NoIndirectDrawing(Component):
    """Marker component to disable indirect drawing for a camera.

    Warning: This should only be added when initially spawning a camera.
    Adding or removing after spawn can result in unspecified behavior.

    Examples:
        >>> commands.spawn(Camera3d(), Camera(), NoIndirectDrawing())
    """

    def __init__(self) -> None:
        """Create a NoIndirectDrawing marker component."""

class StorageBuffer(Asset):
    """A GPU storage buffer asset.

    Storage buffers allow passing large amounts of data to shaders.
    Data can be provided as numpy arrays, lists of floats, or raw bytes.

    Examples:
        ```python
        import numpy as np
        from pybevy.render import StorageBuffer

        # From numpy array
        data = np.array([1.0, 2.0, 3.0, 4.0], dtype=np.float32)
        buf = StorageBuffer(data)

        # From list of floats
        buf = StorageBuffer([1.0, 2.0, 3.0, 4.0])

        # Empty buffer of given byte size
        buf = StorageBuffer.empty(1024)

        # Update contents
        buf.set_data(np.zeros(100, dtype=np.float32))
        ```
    """

    def __init__(
        self, data: npt.NDArray[np.float32] | list[float] | bytes | None = None
    ) -> None:
        """Create a StorageBuffer with optional initial data.

        Args:
            data: Initial buffer data. Can be a numpy array, list of floats, or
                raw bytes. If None, creates an empty default buffer.
        """

    @staticmethod
    def empty(size: int) -> StorageBuffer:
        """Create an empty storage buffer with the given byte size.

        Args:
            size: Buffer size in bytes.
        """

    def set_data(self, data: npt.NDArray[np.float32] | list[float] | bytes) -> None:
        """Update the buffer contents.

        Args:
            data: New buffer data. Can be a numpy array, list of floats, or raw bytes.
        """

    def __len__(self) -> int:
        """Return the buffer size in bytes."""

    def __repr__(self) -> str: ...
