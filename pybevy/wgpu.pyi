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

    def __eq__(self, other: object) -> bool: ...

    D1: ClassVar[TextureDimension]
    D2: ClassVar[TextureDimension]
    D3: ClassVar[TextureDimension]

class VertexFormat:
    """Vertex-buffer element format."""

    def __eq__(self, other: object) -> bool: ...

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
    """Texture format enumeration - controls how pixel data is interpreted."""

    def __eq__(self, other: object) -> bool: ...

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
