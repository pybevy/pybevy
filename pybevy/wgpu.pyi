from typing import TYPE_CHECKING, ClassVar

if TYPE_CHECKING:
    from pybevy.image import ImageSamplerDescriptor

class PowerPreference:
    """GPU power preference for adapter selection.

    Controls whether the system prefers a low-power (integrated) or
    high-performance (discrete) GPU.

    Examples:
        ```python
        from pybevy import RenderPlugin, PowerPreference

        # Use integrated GPU for battery life
        RenderPlugin(power_preference=PowerPreference.LowPower)

        # Use discrete GPU for performance
        RenderPlugin(power_preference=PowerPreference.HighPerformance)
        ```
    """

    None_: ClassVar[PowerPreference]
    """Power usage is not considered when choosing an adapter."""
    LowPower: ClassVar[PowerPreference]
    """Prefer adapter using least power (typically integrated GPU)."""
    HighPerformance: ClassVar[PowerPreference]
    """Prefer adapter with highest performance (typically discrete GPU)."""

    NONE: ClassVar[PowerPreference]
    """Power usage is not considered when choosing an adapter."""
    LOW_POWER: ClassVar[PowerPreference]
    """Prefer adapter using least power (typically integrated GPU)."""
    HIGH_PERFORMANCE: ClassVar[PowerPreference]
    """Prefer adapter with highest performance (typically discrete GPU)."""

class Extent3d:
    """3D texture extent (width, height, depth_or_array_layers)."""

    def __init__(self, width: int, height: int, depth_or_array_layers: int) -> None: ...
    @property
    def width(self) -> int:
        """Width of the texture."""

    @property
    def height(self) -> int:
        """Height of the texture."""

    @property
    def depth_or_array_layers(self) -> int:
        """Depth for 3D textures, or number of array layers for 2D array textures."""

class Face:
    """Face culling mode."""

    Front: ClassVar[Face]
    Back: ClassVar[Face]
    FRONT: ClassVar[Face]
    BACK: ClassVar[Face]

class TextureDimension:
    """Texture dimension - 1D, 2D, or 3D."""

    D1: ClassVar[TextureDimension]
    D2: ClassVar[TextureDimension]
    D3: ClassVar[TextureDimension]

class ImageFilterMode:
    """Image filtering mode for texture sampling."""

    Nearest: ClassVar[ImageFilterMode]
    Linear: ClassVar[ImageFilterMode]

class ImageAddressMode:
    """Image addressing mode for texture coordinates outside [0, 1] range."""

    ClampToEdge: ClassVar[ImageAddressMode]
    Repeat: ClassVar[ImageAddressMode]
    MirrorRepeat: ClassVar[ImageAddressMode]
    ClampToBorder: ClassVar[ImageAddressMode]

class ImageSampler:
    """Texture sampler configuration.

    Describes how a texture is sampled (filtering, wrapping, etc.).

    This is a complex enum with variants:
    - Default: Use the default sampler from ImagePlugin
    - Descriptor: Use a custom ImageSamplerDescriptor
    """

    class Default:
        """Default image sampler variant."""

    class Descriptor:
        """Custom sampler descriptor variant."""

    @staticmethod
    def default() -> ImageSampler:
        """Create a default sampler."""

    @staticmethod
    def descriptor(desc: ImageSamplerDescriptor) -> ImageSampler:
        """Create a sampler with a custom descriptor."""

    @staticmethod
    def linear() -> ImageSampler:
        """Create a sampler with linear filtering for min, mag, and mipmap filters.

        Linear filtering provides smooth interpolation between pixels, ideal for
        most textures and images.

        Returns:
            ImageSampler configured with linear filtering

        Example:
            >>> from pybevy.image import Image
            >>> from pybevy.wgpu import ImageSampler, Extent3d
            >>> img = Image(Extent3d(64, 64, 1))
            >>> img.sampler = ImageSampler.linear()
        """

    @staticmethod
    def nearest() -> ImageSampler:
        """Create a sampler with nearest neighbor filtering for min, mag, and mipmap filters.

        Nearest filtering provides sharp, pixelated appearance - useful for pixel art
        and textures where you want to preserve hard edges.

        Returns:
            ImageSampler configured with nearest neighbor filtering

        Example:
            >>> from pybevy.image import Image
            >>> from pybevy.wgpu import ImageSampler, Extent3d
            >>> img = Image(Extent3d(64, 64, 1))
            >>> img.sampler = ImageSampler.nearest()  # Good for pixel art
        """

    @property
    def is_default(self) -> bool:
        """Whether this is the default sampler."""

    @property
    def mag_filter(self) -> ImageFilterMode | None:
        """Magnification filter mode (None if default sampler)."""

    @property
    def min_filter(self) -> ImageFilterMode | None:
        """Minification filter mode (None if default sampler)."""

    @property
    def mipmap_filter(self) -> ImageFilterMode | None:
        """Mipmap filter mode (None if default sampler)."""

    @property
    def address_mode_u(self) -> ImageAddressMode | None:
        """U coordinate address mode (None if default sampler)."""

    @property
    def address_mode_v(self) -> ImageAddressMode | None:
        """V coordinate address mode (None if default sampler)."""

    @property
    def address_mode_w(self) -> ImageAddressMode | None:
        """W coordinate address mode (None if default sampler)."""

class TextureFormat:
    """Texture format enumeration - controls how pixel data is interpreted."""

    # CamelCase variants (PyO3 enum members)
    R8Unorm: ClassVar[TextureFormat]
    R8Snorm: ClassVar[TextureFormat]
    R8Uint: ClassVar[TextureFormat]
    R8Sint: ClassVar[TextureFormat]
    R16Uint: ClassVar[TextureFormat]
    R16Sint: ClassVar[TextureFormat]
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
    Rg16Float: ClassVar[TextureFormat]
    Rgba8Unorm: ClassVar[TextureFormat]
    Rgba8UnormSrgb: ClassVar[TextureFormat]
    Rgba8Snorm: ClassVar[TextureFormat]
    Rgba8Uint: ClassVar[TextureFormat]
    Rgba8Sint: ClassVar[TextureFormat]
    Bgra8Unorm: ClassVar[TextureFormat]
    Bgra8UnormSrgb: ClassVar[TextureFormat]
    Rgb10a2Unorm: ClassVar[TextureFormat]
    Rg11b10Ufloat: ClassVar[TextureFormat]
    Rg32Uint: ClassVar[TextureFormat]
    Rg32Sint: ClassVar[TextureFormat]
    Rg32Float: ClassVar[TextureFormat]
    Rgba16Uint: ClassVar[TextureFormat]
    Rgba16Sint: ClassVar[TextureFormat]
    Rgba16Float: ClassVar[TextureFormat]
    Rgba32Uint: ClassVar[TextureFormat]
    Rgba32Sint: ClassVar[TextureFormat]
    Rgba32Float: ClassVar[TextureFormat]
    Depth32Float: ClassVar[TextureFormat]
    Depth24Plus: ClassVar[TextureFormat]
    Depth24PlusStencil8: ClassVar[TextureFormat]
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

    # SCREAMING_CASE aliases
    R8_UNORM: ClassVar[TextureFormat]
    R8_SNORM: ClassVar[TextureFormat]
    R8_UINT: ClassVar[TextureFormat]
    R8_SINT: ClassVar[TextureFormat]
    R16_UINT: ClassVar[TextureFormat]
    R16_SINT: ClassVar[TextureFormat]
    R16_FLOAT: ClassVar[TextureFormat]
    RG8_UNORM: ClassVar[TextureFormat]
    RG8_SNORM: ClassVar[TextureFormat]
    RG8_UINT: ClassVar[TextureFormat]
    RG8_SINT: ClassVar[TextureFormat]
    R32_UINT: ClassVar[TextureFormat]
    R32_SINT: ClassVar[TextureFormat]
    R32_FLOAT: ClassVar[TextureFormat]
    RG16_UINT: ClassVar[TextureFormat]
    RG16_SINT: ClassVar[TextureFormat]
    RG16_FLOAT: ClassVar[TextureFormat]
    RGBA8_UNORM: ClassVar[TextureFormat]
    RGBA8_UNORM_SRGB: ClassVar[TextureFormat]
    RGBA8_SNORM: ClassVar[TextureFormat]
    RGBA8_UINT: ClassVar[TextureFormat]
    RGBA8_SINT: ClassVar[TextureFormat]
    BGRA8_UNORM: ClassVar[TextureFormat]
    BGRA8_UNORM_SRGB: ClassVar[TextureFormat]
    RGB10A2_UNORM: ClassVar[TextureFormat]
    RG11B10_UFLOAT: ClassVar[TextureFormat]
    RG32_UINT: ClassVar[TextureFormat]
    RG32_SINT: ClassVar[TextureFormat]
    RG32_FLOAT: ClassVar[TextureFormat]
    RGBA16_UINT: ClassVar[TextureFormat]
    RGBA16_SINT: ClassVar[TextureFormat]
    RGBA16_FLOAT: ClassVar[TextureFormat]
    RGBA32_UINT: ClassVar[TextureFormat]
    RGBA32_SINT: ClassVar[TextureFormat]
    RGBA32_FLOAT: ClassVar[TextureFormat]
    DEPTH32_FLOAT: ClassVar[TextureFormat]
    DEPTH24_PLUS: ClassVar[TextureFormat]
    DEPTH24_PLUS_STENCIL8: ClassVar[TextureFormat]
    BC1_RGBA_UNORM: ClassVar[TextureFormat]
    BC1_RGBA_UNORM_SRGB: ClassVar[TextureFormat]
    BC2_RGBA_UNORM: ClassVar[TextureFormat]
    BC2_RGBA_UNORM_SRGB: ClassVar[TextureFormat]
    BC3_RGBA_UNORM: ClassVar[TextureFormat]
    BC3_RGBA_UNORM_SRGB: ClassVar[TextureFormat]
    BC4_R_UNORM: ClassVar[TextureFormat]
    BC4_R_SNORM: ClassVar[TextureFormat]
    BC5_RG_UNORM: ClassVar[TextureFormat]
    BC5_RG_SNORM: ClassVar[TextureFormat]
    BC6H_RGB_UFLOAT: ClassVar[TextureFormat]
    BC6H_RGB_FLOAT: ClassVar[TextureFormat]
    BC7_RGBA_UNORM: ClassVar[TextureFormat]
    BC7_RGBA_UNORM_SRGB: ClassVar[TextureFormat]
