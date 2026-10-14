import builtins
from collections.abc import Buffer, Iterable
from contextlib import AbstractContextManager
from typing import ClassVar, Literal

from pybevy.app import App, Plugin
from pybevy.array import Array
from pybevy.assets import Asset
from pybevy.ecs import Component
from pybevy.math import Range, Vec2

class ShaderBuffer(Asset):
    """A storage-buffer asset uploaded for shader access.

    Data accepts raw contiguous buffers, uint8 Array, or homogeneous typed
    scalar/vector/@shader_type sequences. Use element=U32 for unsigned data
    and element= for empty sequences. Raw inputs require caller-provided padding. CPU data may be consumed by render extraction;
    these accessors do not perform GPU readback. Use ``with_size`` to reserve GPU allocation size.
    """

    def __copy__(self) -> ShaderBuffer: ...
    def __deepcopy__(self, memo: dict[int, object]) -> ShaderBuffer: ...

    def __init__(
        self, data: Buffer | Array | Iterable[object] | None = None, *, element: object = None, copy_on_resize: bool = False
    ) -> None: ...
    @staticmethod
    def with_size(size: int) -> ShaderBuffer: ...
    def data(self) -> AbstractContextManager[Array]:
        """Borrow read-only CPU bytes; raises RuntimeError when data is absent."""
    def data_mut(self) -> AbstractContextManager[Array]:
        """Borrow writable CPU bytes with asset change tracking; no GPU mapping."""
    def data_copy(self) -> bytes | None:
        """Copy current CPU bytes, or return None if no CPU data is present."""
    def data_len(self) -> int | None:
        """Current CPU byte count, or None; not the GPU allocation size."""
    @property
    def copy_on_resize(self) -> bool: ...
    @copy_on_resize.setter
    def copy_on_resize(self, value: bool) -> None: ...
    def set_data(self, value: Buffer | Array | Iterable[object] | None, *, element: object = None) -> None:
        """Pack typed elements or replace raw bytes atomically; None clears CPU data."""
    def resize(self, size: int) -> None: ...
    def resize_in_place(self, size: int) -> None: ...

class TextureDescriptor:
    """Image texture descriptor.

    Obtain from Image.texture_descriptor; copy.copy() creates a mutable copy.
    """

    @property
    def mip_level_count(self) -> int: ...
    @mip_level_count.setter
    def mip_level_count(self, value: int) -> None: ...
    @property
    def dimension(self) -> TextureDimension: ...
    @dimension.setter
    def dimension(self, value: TextureDimension) -> None: ...
    @property
    def format(self) -> TextureFormat: ...
    @format.setter
    def format(self, value: TextureFormat) -> None: ...
    @property
    def usage(self) -> TextureUsages: ...
    @usage.setter
    def usage(self, value: TextureUsages) -> None: ...
    def __copy__(self) -> TextureDescriptor: ...

class TextureUsages:
    """Mutable texture descriptor usage flags. Combine flags with `|`."""

    COPY_SRC: ClassVar[TextureUsages]
    COPY_DST: ClassVar[TextureUsages]
    TEXTURE_BINDING: ClassVar[TextureUsages]
    STORAGE_BINDING: ClassVar[TextureUsages]
    RENDER_ATTACHMENT: ClassVar[TextureUsages]
    STORAGE_ATOMIC: ClassVar[TextureUsages]
    TRANSIENT: ClassVar[TextureUsages]

    def __init__(self) -> None:
        """Create an empty set of texture usage flags."""

    def __or__(self, other: TextureUsages) -> TextureUsages: ...
    def __eq__(self, other: object) -> bool: ...
    def __copy__(self) -> TextureUsages: ...
    def insert(self, other: TextureUsages) -> None: ...
    def remove(self, other: TextureUsages) -> None: ...
    def toggle(self, other: TextureUsages) -> None: ...
    def set(self, other: TextureUsages, value: bool) -> None: ...
    def contains(self, other: TextureUsages) -> bool:
        """Whether all flags in other are present."""

class Extent3d:
    """3D texture extent (width, height, depth_or_array_layers)."""

    def __init__(self, *, width: int = 1, height: int = 1, depth_or_array_layers: int = 1) -> None: ...
    @property
    def width(self) -> int:
        """Width of the texture."""

    @width.setter
    def width(self, value: int) -> None: ...

    @property
    def height(self) -> int:
        """Height of the texture."""

    @height.setter
    def height(self, value: int) -> None: ...

    @property
    def depth_or_array_layers(self) -> int:
        """Depth for 3D textures, or number of array layers for 2D array textures."""

    @depth_or_array_layers.setter
    def depth_or_array_layers(self, value: int) -> None: ...

class TextureDimension:
    """Texture dimension: 1D, 2D, or 3D."""

    def __copy__(self) -> TextureDimension: ...
    def __deepcopy__(self, memo: dict[int, object]) -> TextureDimension: ...


    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

    D1: ClassVar[TextureDimension]
    D2: ClassVar[TextureDimension]
    D3: ClassVar[TextureDimension]

class VertexFormat:
    """Vertex-buffer element format."""

    def __copy__(self) -> VertexFormat: ...
    def __deepcopy__(self, memo: dict[int, object]) -> VertexFormat: ...


    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

    Uint8: ClassVar[VertexFormat]
    Uint8x2: ClassVar[VertexFormat]
    Uint8x4: ClassVar[VertexFormat]
    Sint8: ClassVar[VertexFormat]
    Sint8x2: ClassVar[VertexFormat]
    Sint8x4: ClassVar[VertexFormat]
    Unorm8: ClassVar[VertexFormat]
    Unorm8x2: ClassVar[VertexFormat]
    Unorm8x4: ClassVar[VertexFormat]
    Snorm8: ClassVar[VertexFormat]
    Snorm8x2: ClassVar[VertexFormat]
    Snorm8x4: ClassVar[VertexFormat]
    Uint16: ClassVar[VertexFormat]
    Uint16x2: ClassVar[VertexFormat]
    Uint16x4: ClassVar[VertexFormat]
    Sint16: ClassVar[VertexFormat]
    Sint16x2: ClassVar[VertexFormat]
    Sint16x4: ClassVar[VertexFormat]
    Unorm16: ClassVar[VertexFormat]
    Unorm16x2: ClassVar[VertexFormat]
    Unorm16x4: ClassVar[VertexFormat]
    Snorm16: ClassVar[VertexFormat]
    Snorm16x2: ClassVar[VertexFormat]
    Snorm16x4: ClassVar[VertexFormat]
    Float16: ClassVar[VertexFormat]
    Float16x2: ClassVar[VertexFormat]
    Float16x4: ClassVar[VertexFormat]
    Float32: ClassVar[VertexFormat]
    Float32x2: ClassVar[VertexFormat]
    Float32x3: ClassVar[VertexFormat]
    Float32x4: ClassVar[VertexFormat]
    Uint32: ClassVar[VertexFormat]
    Uint32x2: ClassVar[VertexFormat]
    Uint32x3: ClassVar[VertexFormat]
    Uint32x4: ClassVar[VertexFormat]
    Sint32: ClassVar[VertexFormat]
    Sint32x2: ClassVar[VertexFormat]
    Sint32x3: ClassVar[VertexFormat]
    Sint32x4: ClassVar[VertexFormat]
    Float64: ClassVar[VertexFormat]
    Float64x2: ClassVar[VertexFormat]
    Float64x3: ClassVar[VertexFormat]
    Float64x4: ClassVar[VertexFormat]
    Unorm10_10_10_2: ClassVar[VertexFormat]
    Unorm8x4Bgra: ClassVar[VertexFormat]

class TextureFormat:
    """Texture format controlling how pixel data is interpreted."""

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

    R8Unorm: ClassVar[TextureFormat]
    R8Snorm: ClassVar[TextureFormat]
    R8Uint: ClassVar[TextureFormat]
    R8Sint: ClassVar[TextureFormat]
    R16Uint: ClassVar[TextureFormat]
    R16Sint: ClassVar[TextureFormat]
    R16Unorm: ClassVar[TextureFormat]
    R16Snorm: ClassVar[TextureFormat]
    R16Float: ClassVar[TextureFormat]
    Rg8Unorm: ClassVar[TextureFormat]
    Rg8Snorm: ClassVar[TextureFormat]
    Rg8Uint: ClassVar[TextureFormat]
    Rg8Sint: ClassVar[TextureFormat]
    R32Uint: ClassVar[TextureFormat]
    R32Sint: ClassVar[TextureFormat]
    R32Float: ClassVar[TextureFormat]
    Rg16Uint: ClassVar[TextureFormat]
    Rg16Sint: ClassVar[TextureFormat]
    Rg16Unorm: ClassVar[TextureFormat]
    Rg16Snorm: ClassVar[TextureFormat]
    Rg16Float: ClassVar[TextureFormat]
    Rgba8Unorm: ClassVar[TextureFormat]
    Rgba8UnormSrgb: ClassVar[TextureFormat]
    Rgba8Snorm: ClassVar[TextureFormat]
    Rgba8Uint: ClassVar[TextureFormat]
    Rgba8Sint: ClassVar[TextureFormat]
    Bgra8Unorm: ClassVar[TextureFormat]
    Bgra8UnormSrgb: ClassVar[TextureFormat]
    Rgb9e5Ufloat: ClassVar[TextureFormat]
    Rgb10a2Uint: ClassVar[TextureFormat]
    Rgb10a2Unorm: ClassVar[TextureFormat]
    Rg11b10Ufloat: ClassVar[TextureFormat]
    R64Uint: ClassVar[TextureFormat]
    Rg32Uint: ClassVar[TextureFormat]
    Rg32Sint: ClassVar[TextureFormat]
    Rg32Float: ClassVar[TextureFormat]
    Rgba16Uint: ClassVar[TextureFormat]
    Rgba16Sint: ClassVar[TextureFormat]
    Rgba16Unorm: ClassVar[TextureFormat]
    Rgba16Snorm: ClassVar[TextureFormat]
    Rgba16Float: ClassVar[TextureFormat]
    Rgba32Uint: ClassVar[TextureFormat]
    Rgba32Sint: ClassVar[TextureFormat]
    Rgba32Float: ClassVar[TextureFormat]
    Stencil8: ClassVar[TextureFormat]
    Depth16Unorm: ClassVar[TextureFormat]
    Depth24Plus: ClassVar[TextureFormat]
    Depth24PlusStencil8: ClassVar[TextureFormat]
    Depth32Float: ClassVar[TextureFormat]
    Depth32FloatStencil8: ClassVar[TextureFormat]
    NV12: ClassVar[TextureFormat]
    P010: ClassVar[TextureFormat]
    Bc1RgbaUnorm: ClassVar[TextureFormat]
    Bc1RgbaUnormSrgb: ClassVar[TextureFormat]
    Bc2RgbaUnorm: ClassVar[TextureFormat]
    Bc2RgbaUnormSrgb: ClassVar[TextureFormat]
    Bc3RgbaUnorm: ClassVar[TextureFormat]
    Bc3RgbaUnormSrgb: ClassVar[TextureFormat]
    Bc4RUnorm: ClassVar[TextureFormat]
    Bc4RSnorm: ClassVar[TextureFormat]
    Bc5RgUnorm: ClassVar[TextureFormat]
    Bc5RgSnorm: ClassVar[TextureFormat]
    Bc6hRgbUfloat: ClassVar[TextureFormat]
    Bc6hRgbFloat: ClassVar[TextureFormat]
    Bc7RgbaUnorm: ClassVar[TextureFormat]
    Bc7RgbaUnormSrgb: ClassVar[TextureFormat]
    Etc2Rgb8Unorm: ClassVar[TextureFormat]
    Etc2Rgb8UnormSrgb: ClassVar[TextureFormat]
    Etc2Rgb8A1Unorm: ClassVar[TextureFormat]
    Etc2Rgb8A1UnormSrgb: ClassVar[TextureFormat]
    Etc2Rgba8Unorm: ClassVar[TextureFormat]
    Etc2Rgba8UnormSrgb: ClassVar[TextureFormat]
    EacR11Unorm: ClassVar[TextureFormat]
    EacR11Snorm: ClassVar[TextureFormat]
    EacRg11Unorm: ClassVar[TextureFormat]
    EacRg11Snorm: ClassVar[TextureFormat]

class PowerPreference:
    """GPU power preference for adapter selection."""

    def __copy__(self) -> PowerPreference: ...
    def __deepcopy__(self, memo: dict[int, object]) -> PowerPreference: ...


    None_: ClassVar[PowerPreference]
    LowPower: ClassVar[PowerPreference]
    HighPerformance: ClassVar[PowerPreference]

    def __hash__(self) -> int: ...

class Face:
    """Face culling mode."""

    def __copy__(self) -> Face: ...
    def __deepcopy__(self, memo: dict[int, object]) -> Face: ...


    Front: ClassVar[Face]
    Back: ClassVar[Face]

    def __hash__(self) -> int: ...

class TextureAspect:
    """Which aspect of a texture a view covers."""

    All: ClassVar[TextureAspect]
    StencilOnly: ClassVar[TextureAspect]
    DepthOnly: ClassVar[TextureAspect]
    Plane0: ClassVar[TextureAspect]
    Plane1: ClassVar[TextureAspect]
    Plane2: ClassVar[TextureAspect]

    def __copy__(self) -> TextureAspect: ...
    def __deepcopy__(self, memo: dict[int, object]) -> TextureAspect: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class TextureViewDescriptor:
    """Describes how an image's texture view is created.

    Obtain from Image.texture_view_descriptor; copy.copy() creates a mutable copy.
    """

    def __init__(
        self,
        *,
        label: str | None = None,
        format: TextureFormat | None = None,
        dimension: TextureViewDimension | None = None,
        usage: TextureUsages | None = None,
        aspect: TextureAspect = TextureAspect.All,
        base_mip_level: int = 0,
        mip_level_count: int | None = None,
        base_array_layer: int = 0,
        array_layer_count: int | None = None,
    ) -> None: ...
    @property
    def label(self) -> str | None:
        """Debug label; each distinct label is interned for the process lifetime."""
    @label.setter
    def label(self, value: str | None) -> None: ...
    @property
    def format(self) -> TextureFormat | None: ...
    @format.setter
    def format(self, value: TextureFormat | None) -> None: ...
    @property
    def dimension(self) -> TextureViewDimension | None: ...
    @dimension.setter
    def dimension(self, value: TextureViewDimension | None) -> None: ...
    @property
    def usage(self) -> TextureUsages | None: ...
    @usage.setter
    def usage(self, value: TextureUsages | None) -> None: ...
    @property
    def aspect(self) -> TextureAspect: ...
    @aspect.setter
    def aspect(self, value: TextureAspect) -> None: ...
    @property
    def base_mip_level(self) -> int: ...
    @base_mip_level.setter
    def base_mip_level(self, value: int) -> None: ...
    @property
    def mip_level_count(self) -> int | None: ...
    @mip_level_count.setter
    def mip_level_count(self, value: int | None) -> None: ...
    @property
    def base_array_layer(self) -> int: ...
    @base_array_layer.setter
    def base_array_layer(self, value: int) -> None: ...
    @property
    def array_layer_count(self) -> int | None: ...
    @array_layer_count.setter
    def array_layer_count(self, value: int | None) -> None: ...
    def __copy__(self) -> TextureViewDescriptor: ...

class TextureViewDimension:
    """Dimension used when interpreting a texture through a view."""

    def __copy__(self) -> TextureViewDimension: ...
    def __deepcopy__(self, memo: dict[int, object]) -> TextureViewDimension: ...


    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

    D1: ClassVar[TextureViewDimension]
    D2: ClassVar[TextureViewDimension]
    D2Array: ClassVar[TextureViewDimension]
    Cube: ClassVar[TextureViewDimension]
    CubeArray: ClassVar[TextureViewDimension]
    D3: ClassVar[TextureViewDimension]

class TexturePlugin(Plugin):
    """Registers image codecs and texture resources.

    CPU image loading needs AssetPlugin and ImagePlugin before finish().
    RenderPlugin already includes this plugin.
    """
    def __init__(self) -> None: ...

class Backends:
    """Mutable set of wgpu backends. Combine flags with `|`."""

    NOOP: ClassVar[Backends]
    VULKAN: ClassVar[Backends]
    GL: ClassVar[Backends]
    METAL: ClassVar[Backends]
    DX12: ClassVar[Backends]
    BROWSER_WEBGPU: ClassVar[Backends]
    PRIMARY: ClassVar[Backends]
    SECONDARY: ClassVar[Backends]

    def __init__(self) -> None:
        """Create Bevy's default set: every backend."""

    def __or__(self, other: Backends) -> Backends: ...
    def __eq__(self, other: object) -> bool: ...
    def __copy__(self) -> Backends: ...
    def insert(self, other: Backends) -> None: ...
    def remove(self, other: Backends) -> None: ...
    def toggle(self, other: Backends) -> None: ...
    def set(self, other: Backends, value: bool) -> None: ...
    def contains(self, other: Backends) -> bool:
        """Whether all backends in other are present."""

class WgpuSettingsPriority:
    """Which features and limits wgpu requests from the adapter."""

    WebGPU: ClassVar[WgpuSettingsPriority]
    Functionality: ClassVar[WgpuSettingsPriority]
    WebGL2: ClassVar[WgpuSettingsPriority]

    def __copy__(self) -> WgpuSettingsPriority: ...
    def __deepcopy__(self, memo: dict[int, object]) -> WgpuSettingsPriority: ...
    def __eq__(self, other: object) -> bool: ...

class Gles3MinorVersion:
    """GLES 3 minor version requested by the GL backend."""

    Automatic: ClassVar[Gles3MinorVersion]
    Version0: ClassVar[Gles3MinorVersion]
    Version1: ClassVar[Gles3MinorVersion]
    Version2: ClassVar[Gles3MinorVersion]

    def __copy__(self) -> Gles3MinorVersion: ...
    def __deepcopy__(self, memo: dict[int, object]) -> Gles3MinorVersion: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class WgpuSettings:
    """Settings for the automatic wgpu renderer setup.

    Omitted arguments keep Bevy's defaults, which honor the WGPU_BACKEND,
    WGPU_POWER_PREF, WGPU_SETTINGS_PRIO and WGPU_GLES_MINOR_VERSION
    environment variables. `backends=None` disables renderer creation.
    Features, limits, DX12 compiler, instance flags, memory hints and memory
    budget thresholds keep Bevy's defaults and are not exposed.
    """

    def __init__(
        self,
        *,
        device_label: str | None = ...,
        backends: Backends | None = ...,
        power_preference: PowerPreference = ...,
        priority: WgpuSettingsPriority = ...,
        gles3_minor_version: Gles3MinorVersion = ...,
        force_fallback_adapter: bool = False,
        adapter_name: str | None = None,
    ) -> None: ...
    @property
    def device_label(self) -> str | None: ...
    @device_label.setter
    def device_label(self, value: str | None) -> None: ...
    @property
    def backends(self) -> Backends | None: ...
    @backends.setter
    def backends(self, value: Backends | None) -> None: ...
    @property
    def power_preference(self) -> PowerPreference: ...
    @power_preference.setter
    def power_preference(self, value: PowerPreference) -> None: ...
    @property
    def priority(self) -> WgpuSettingsPriority: ...
    @priority.setter
    def priority(self, value: WgpuSettingsPriority) -> None: ...
    @property
    def gles3_minor_version(self) -> Gles3MinorVersion: ...
    @gles3_minor_version.setter
    def gles3_minor_version(self, value: Gles3MinorVersion) -> None: ...
    @property
    def force_fallback_adapter(self) -> bool: ...
    @force_fallback_adapter.setter
    def force_fallback_adapter(self, value: bool) -> None: ...
    @property
    def adapter_name(self) -> str | None: ...
    @adapter_name.setter
    def adapter_name(self, value: str | None) -> None: ...
    def __copy__(self) -> WgpuSettings: ...

class RenderCreation:
    """How the renderer's GPU resources are created.

    Only `RenderCreation.Automatic(settings)` is available from Python; Bevy's
    `Manual` variant carries live GPU device handles.
    """

    class Automatic(RenderCreation):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: WgpuSettings) -> None: ...
        @property
        def value(self) -> WgpuSettings:
            """A read-only snapshot of the settings; construct a new value to change them."""

class RenderPlugin(Plugin):
    """Configures the rendering pipeline.

    Examples:
        ```python
        from pybevy.app import DefaultPlugins
        from pybevy.render import PowerPreference, RenderCreation, RenderPlugin, WgpuSettings

        # Low power GPU
        app.add_plugins(
            DefaultPlugins().set(RenderPlugin(
                render_creation=RenderCreation.Automatic(
                    WgpuSettings(power_preference=PowerPreference.LowPower)
                )
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
        *,
        render_creation: RenderCreation = ...,
        synchronous_pipeline_compilation: bool = False
    ) -> None:
        """Create a RenderPlugin with optional configuration.

        Args:
            render_creation: Renderer setup; defaults to
                RenderCreation.Automatic(WgpuSettings()).
            synchronous_pipeline_compilation: If True, disables async pipeline compilation.
                No effect on macOS, Wasm, or iOS.
        """

    def build(self, app: App) -> None: ...

    @property
    def render_creation(self) -> RenderCreation: ...

    @property
    def synchronous_pipeline_compilation(self) -> bool: ...

class ColorGradingSection:
    """Color grading values applied to a specific tonal range (shadows, midtones, or highlights).

    Supports borrowed access: mutations to sections obtained from ColorGrading.shadows etc.
    will persist back to the parent component.
    """

    def __init__(
        self,
        *,
        saturation: float = 1.0,
        contrast: float = 1.0,
        gamma: float = 1.0,
        gain: float = 1.0,
        lift: float = 0.0
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
        *,
        exposure: float = 0.0,
        temperature: float = 0.0,
        tint: float = 0.0,
        hue: float = 0.0,
        post_saturation: float = 1.0,
        midtones_range: Range = Range(0.2, 0.7)
    ) -> None: ...

    @property
    def exposure(self) -> float:
        """Exposure value (EV) offset, measured in stops."""
    @exposure.setter
    def exposure(self, value: float) -> None: ...

    @property
    def temperature(self) -> float:
        """Adjustment to the CIE 1931 chromaticity x value. Positive values
        make colors redder, negative bluer, with no effect on luminance. The
        white point sits at x = 0.3127, so useful values are roughly -0.1 to
        0.1."""
    @temperature.setter
    def temperature(self, value: float) -> None: ...

    @property
    def tint(self) -> float:
        """Adjustment to the CIE 1931 chromaticity y value. Positive values
        make colors more magenta, negative greener, with no effect on
        luminance. Same scale as `temperature`: roughly -0.1 to 0.1."""
    @tint.setter
    def tint(self, value: float) -> None: ...

    @property
    def hue(self) -> float:
        """Hue adjustment in radians. No effect on saturation or brightness."""
    @hue.setter
    def hue(self, value: float) -> None: ...

    @property
    def post_saturation(self) -> float:
        """Saturation applied after tonemapping. Below 1.0 desaturates, 0.0
        gives BT.709 grayscale, above 1.0 increases saturation."""
    @post_saturation.setter
    def post_saturation(self, value: float) -> None: ...

    @property
    def midtones_range(self) -> Range: ...
    @midtones_range.setter
    def midtones_range(self, value: Range) -> None: ...

class ColorGrading(Component):
    """Filmic color grading for camera post-processing.

    Provides control over exposure, temperature, tint, hue, saturation,
    and section-specific adjustments for shadows, midtones, and highlights.

    On non-HDR cameras, only global exposure and post-saturation apply. Other
    fields require Hdr and a tonemapper other than Tonemapping.None_, which is
    what Camera2d defaults to.
    """

    def __init__(
        self,
        *,
        global_: ColorGradingGlobal = ...,
        shadows: ColorGradingSection = ...,
        midtones: ColorGradingSection = ...,
        highlights: ColorGradingSection = ...
    ) -> None: ...

    @property
    def global_(self) -> ColorGradingGlobal: ...
    @global_.setter
    def global_(self, value: ColorGradingGlobal) -> None: ...

    @property
    def shadows(self) -> ColorGradingSection: ...
    @shadows.setter
    def shadows(self, value: ColorGradingSection) -> None: ...

    @property
    def midtones(self) -> ColorGradingSection: ...
    @midtones.setter
    def midtones(self, value: ColorGradingSection) -> None: ...

    @property
    def highlights(self) -> ColorGradingSection: ...
    @highlights.setter
    def highlights(self, value: ColorGradingSection) -> None: ...

    @staticmethod
    def with_identical_sections(
        global_: ColorGradingGlobal, section: ColorGradingSection
    ) -> ColorGrading:
        """Create ColorGrading with identical shadow, midtone, and highlight sections.

        Args:
            global_: Global color grading settings
            section: Section to use for shadows, midtones, and highlights

        Returns:
            ColorGrading with the same section applied to all tonal ranges
        """

class Msaa(Component):
    """Multi-sample anti-aliasing configuration.

    MSAA smooths jagged edges by sampling multiple points per pixel.
    Higher sample counts produce smoother edges at the cost of performance.

    Example:
        ```python
        from pybevy.render import Msaa

        # Disable MSAA for maximum performance
        commands.spawn(Camera3d(), Msaa.Off)

        # Use 4x MSAA (default)
        commands.spawn(Camera3d(), Msaa.Sample4)
        ```
    """

    def __init__(self) -> None: ...

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
            ValueError: If samples is not 1, 2, 4, or 8
        """

class TemporalJitter(Component):
    """A subpixel offset to jitter a perspective camera's frustum.

    Useful for temporal rendering techniques like TAA (Temporal Anti-Aliasing).
    The offset should be in the range [-0.5, 0.5].

    Examples:
        >>> jitter = TemporalJitter(offset=Vec2(0.25, -0.25))
        >>> commands.spawn(Camera3d(), Camera(), jitter)
    """

    def __init__(self, *, offset: Vec2 = Vec2.ZERO) -> None:
        """Create a new TemporalJitter component.

        Args:
            offset: Subpixel offset in range [-0.5, 0.5]. Default is zero (no jitter).

        Raises:
            ValueError: If either component is outside [-0.5, 0.5]
        """

    @property
    def offset(self) -> Vec2:
        """Get the current jitter offset."""

    @offset.setter
    def offset(self, value: Vec2) -> None:
        """Set the jitter offset. Raises ValueError outside [-0.5, 0.5]."""

class MipBias(Component):
    """Camera component specifying a mip bias for texture sampling.

    Often used in conjunction with antialiasing post-process effects to reduce
    texture blurriness. A negative value (the default of -1.0) samples sharper
    mip levels. It only affects images whose mip_level_count is greater than 1.

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

    A DepthPrepass component is required on the same camera. Using
    OcclusionCulling without it can prevent the camera from rendering.

    Examples:
        >>> commands.spawn(Camera3d(), DepthPrepass(), OcclusionCulling())
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

class ShaderStage:
    """
    Stage of the programmable pipeline a GLSL source belongs to.

    Passed to `Shader.from_glsl` to say how the source should be compiled.

    Example:
        ```python
        from pybevy.shader import Shader
        from pybevy.render import ShaderStage

        shader = Shader.from_glsl(source, ShaderStage.Fragment, "custom_shader")
        ```
    """

    def __copy__(self) -> ShaderStage: ...
    def __deepcopy__(self, memo: dict[int, object]) -> ShaderStage: ...


    Vertex: ClassVar[ShaderStage]
    """A vertex shader, in a render pipeline."""

    Task: ClassVar[ShaderStage]
    """A task shader, in a mesh render pipeline."""

    Mesh: ClassVar[ShaderStage]
    """A mesh shader, in a mesh render pipeline."""

    Fragment: ClassVar[ShaderStage]
    """A fragment shader, in a render pipeline."""

    Compute: ClassVar[ShaderStage]
    """A compute pipeline shader."""

    RayGeneration: ClassVar[ShaderStage]
    """A ray generation shader, in a ray tracing pipeline."""

    Miss: ClassVar[ShaderStage]
    """A miss shader, in a ray tracing pipeline."""

    AnyHit: ClassVar[ShaderStage]
    """An any hit shader, in a ray tracing pipeline."""

    ClosestHit: ClassVar[ShaderStage]
    """A closest hit shader, in a ray tracing pipeline."""

    def __eq__(self, other: object) -> builtins.bool: ...
    def __ne__(self, other: object) -> builtins.bool: ...
    def __hash__(self) -> builtins.int: ...
