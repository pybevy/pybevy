from typing import ClassVar

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

class TextureDimension:
    """Texture dimension - 1D, 2D, or 3D."""

    D1: ClassVar[TextureDimension]
    D2: ClassVar[TextureDimension]
    D3: ClassVar[TextureDimension]

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
