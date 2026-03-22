use bevy::render::render_resource::TextureFormat;
use pyo3::prelude::*;

#[pyclass(name = "TextureFormat")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyTextureFormat {
    R8Unorm,
    R8Snorm,
    R8Uint,
    R8Sint,
    R16Uint,
    R16Sint,
    R16Float,
    Rg8Unorm,
    Rg8Snorm,
    Rg8Uint,
    Rg8Sint,
    R32Uint,
    R32Sint,
    R32Float,
    Rg16Uint,
    Rg16Sint,
    Rg16Float,
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Rgba8Snorm,
    Rgba8Uint,
    Rgba8Sint,
    Bgra8Unorm,
    Bgra8UnormSrgb,
    Rgb10a2Unorm,
    Rg11b10Ufloat,
    Rg32Uint,
    Rg32Sint,
    Rg32Float,
    Rgba16Uint,
    Rgba16Sint,
    Rgba16Float,
    Rgba32Uint,
    Rgba32Sint,
    Rgba32Float,
    Depth32Float,
    Depth24Plus,
    Depth24PlusStencil8,
    Bc1RgbaUnorm,
    Bc1RgbaUnormSrgb,
    Bc2RgbaUnorm,
    Bc2RgbaUnormSrgb,
    Bc3RgbaUnorm,
    Bc3RgbaUnormSrgb,
    Bc4RUnorm,
    Bc4RSnorm,
    Bc5RgUnorm,
    Bc5RgSnorm,
    Bc6hRgbUfloat,
    Bc6hRgbFloat,
    Bc7RgbaUnorm,
    Bc7RgbaUnormSrgb,
}

#[pymethods]
impl PyTextureFormat {
    #[classattr]
    pub const R8_UNORM: Self = PyTextureFormat::R8Unorm;
    #[classattr]
    pub const R8_SNORM: Self = PyTextureFormat::R8Snorm;
    #[classattr]
    pub const R8_UINT: Self = PyTextureFormat::R8Uint;
    #[classattr]
    pub const R8_SINT: Self = PyTextureFormat::R8Sint;
    #[classattr]
    pub const R16_UINT: Self = PyTextureFormat::R16Uint;
    #[classattr]
    pub const R16_SINT: Self = PyTextureFormat::R16Sint;
    #[classattr]
    pub const R16_FLOAT: Self = PyTextureFormat::R16Float;
    #[classattr]
    pub const RG8_UNORM: Self = PyTextureFormat::Rg8Unorm;
    #[classattr]
    pub const RG8_SNORM: Self = PyTextureFormat::Rg8Snorm;
    #[classattr]
    pub const RG8_UINT: Self = PyTextureFormat::Rg8Uint;
    #[classattr]
    pub const RG8_SINT: Self = PyTextureFormat::Rg8Sint;
    #[classattr]
    pub const R32_UINT: Self = PyTextureFormat::R32Uint;
    #[classattr]
    pub const R32_SINT: Self = PyTextureFormat::R32Sint;
    #[classattr]
    pub const R32_FLOAT: Self = PyTextureFormat::R32Float;
    #[classattr]
    pub const RG16_UINT: Self = PyTextureFormat::Rg16Uint;
    #[classattr]
    pub const RG16_SINT: Self = PyTextureFormat::Rg16Sint;
    #[classattr]
    pub const RG16_FLOAT: Self = PyTextureFormat::Rg16Float;
    #[classattr]
    pub const RGBA8_UNORM: Self = PyTextureFormat::Rgba8Unorm;
    #[classattr]
    pub const RGBA8_UNORM_SRGB: Self = PyTextureFormat::Rgba8UnormSrgb;
    #[classattr]
    pub const RGBA8_SNORM: Self = PyTextureFormat::Rgba8Snorm;
    #[classattr]
    pub const RGBA8_UINT: Self = PyTextureFormat::Rgba8Uint;
    #[classattr]
    pub const RGBA8_SINT: Self = PyTextureFormat::Rgba8Sint;
    #[classattr]
    pub const BGRA8_UNORM: Self = PyTextureFormat::Bgra8Unorm;
    #[classattr]
    pub const BGRA8_UNORM_SRGB: Self = PyTextureFormat::Bgra8UnormSrgb;
    #[classattr]
    pub const RGB10A2_UNORM: Self = PyTextureFormat::Rgb10a2Unorm;
    #[classattr]
    pub const RG11B10_UFLOAT: Self = PyTextureFormat::Rg11b10Ufloat;
    #[classattr]
    pub const RG32_UINT: Self = PyTextureFormat::Rg32Uint;
    #[classattr]
    pub const RG32_SINT: Self = PyTextureFormat::Rg32Sint;
    #[classattr]
    pub const RG32_FLOAT: Self = PyTextureFormat::Rg32Float;
    #[classattr]
    pub const RGBA16_UINT: Self = PyTextureFormat::Rgba16Uint;
    #[classattr]
    pub const RGBA16_SINT: Self = PyTextureFormat::Rgba16Sint;
    #[classattr]
    pub const RGBA16_FLOAT: Self = PyTextureFormat::Rgba16Float;
    #[classattr]
    pub const RGBA32_UINT: Self = PyTextureFormat::Rgba32Uint;
    #[classattr]
    pub const RGBA32_SINT: Self = PyTextureFormat::Rgba32Sint;
    #[classattr]
    pub const RGBA32_FLOAT: Self = PyTextureFormat::Rgba32Float;
    #[classattr]
    pub const DEPTH32_FLOAT: Self = PyTextureFormat::Depth32Float;
    #[classattr]
    pub const DEPTH24_PLUS: Self = PyTextureFormat::Depth24Plus;
    #[classattr]
    pub const DEPTH24_PLUS_STENCIL8: Self = PyTextureFormat::Depth24PlusStencil8;
    #[classattr]
    pub const BC1_RGBA_UNORM: Self = PyTextureFormat::Bc1RgbaUnorm;
    #[classattr]
    pub const BC1_RGBA_UNORM_SRGB: Self = PyTextureFormat::Bc1RgbaUnormSrgb;
    #[classattr]
    pub const BC2_RGBA_UNORM: Self = PyTextureFormat::Bc2RgbaUnorm;
    #[classattr]
    pub const BC2_RGBA_UNORM_SRGB: Self = PyTextureFormat::Bc2RgbaUnormSrgb;
    #[classattr]
    pub const BC3_RGBA_UNORM: Self = PyTextureFormat::Bc3RgbaUnorm;
    #[classattr]
    pub const BC3_RGBA_UNORM_SRGB: Self = PyTextureFormat::Bc3RgbaUnormSrgb;
    #[classattr]
    pub const BC4_R_UNORM: Self = PyTextureFormat::Bc4RUnorm;
    #[classattr]
    pub const BC4_R_SNORM: Self = PyTextureFormat::Bc4RSnorm;
    #[classattr]
    pub const BC5_RG_UNORM: Self = PyTextureFormat::Bc5RgUnorm;
    #[classattr]
    pub const BC5_RG_SNORM: Self = PyTextureFormat::Bc5RgSnorm;
    #[classattr]
    pub const BC6H_RGB_UFLOAT: Self = PyTextureFormat::Bc6hRgbUfloat;
    #[classattr]
    pub const BC6H_RGB_FLOAT: Self = PyTextureFormat::Bc6hRgbFloat;
    #[classattr]
    pub const BC7_RGBA_UNORM: Self = PyTextureFormat::Bc7RgbaUnorm;
    #[classattr]
    pub const BC7_RGBA_UNORM_SRGB: Self = PyTextureFormat::Bc7RgbaUnormSrgb;
}

impl From<PyTextureFormat> for TextureFormat {
    fn from(format: PyTextureFormat) -> Self {
        match format {
            PyTextureFormat::R8Unorm => TextureFormat::R8Unorm,
            PyTextureFormat::R8Snorm => TextureFormat::R8Snorm,
            PyTextureFormat::R8Uint => TextureFormat::R8Uint,
            PyTextureFormat::R8Sint => TextureFormat::R8Sint,
            PyTextureFormat::R16Uint => TextureFormat::R16Uint,
            PyTextureFormat::R16Sint => TextureFormat::R16Sint,
            PyTextureFormat::R16Float => TextureFormat::R16Float,
            PyTextureFormat::Rg8Unorm => TextureFormat::Rg8Unorm,
            PyTextureFormat::Rg8Snorm => TextureFormat::Rg8Snorm,
            PyTextureFormat::Rg8Uint => TextureFormat::Rg8Uint,
            PyTextureFormat::Rg8Sint => TextureFormat::Rg8Sint,
            PyTextureFormat::R32Uint => TextureFormat::R32Uint,
            PyTextureFormat::R32Sint => TextureFormat::R32Sint,
            PyTextureFormat::R32Float => TextureFormat::R32Float,
            PyTextureFormat::Rg16Uint => TextureFormat::Rg16Uint,
            PyTextureFormat::Rg16Sint => TextureFormat::Rg16Sint,
            PyTextureFormat::Rg16Float => TextureFormat::Rg16Float,
            PyTextureFormat::Rgba8Unorm => TextureFormat::Rgba8Unorm,
            PyTextureFormat::Rgba8UnormSrgb => TextureFormat::Rgba8UnormSrgb,
            PyTextureFormat::Rgba8Snorm => TextureFormat::Rgba8Snorm,
            PyTextureFormat::Rgba8Uint => TextureFormat::Rgba8Uint,
            PyTextureFormat::Rgba8Sint => TextureFormat::Rgba8Sint,
            PyTextureFormat::Bgra8Unorm => TextureFormat::Bgra8Unorm,
            PyTextureFormat::Bgra8UnormSrgb => TextureFormat::Bgra8UnormSrgb,
            PyTextureFormat::Rgb10a2Unorm => TextureFormat::Rgb10a2Unorm,
            PyTextureFormat::Rg11b10Ufloat => TextureFormat::Rg11b10Ufloat,
            PyTextureFormat::Rg32Uint => TextureFormat::Rg32Uint,
            PyTextureFormat::Rg32Sint => TextureFormat::Rg32Sint,
            PyTextureFormat::Rg32Float => TextureFormat::Rg32Float,
            PyTextureFormat::Rgba16Uint => TextureFormat::Rgba16Uint,
            PyTextureFormat::Rgba16Sint => TextureFormat::Rgba16Sint,
            PyTextureFormat::Rgba16Float => TextureFormat::Rgba16Float,
            PyTextureFormat::Rgba32Uint => TextureFormat::Rgba32Uint,
            PyTextureFormat::Rgba32Sint => TextureFormat::Rgba32Sint,
            PyTextureFormat::Rgba32Float => TextureFormat::Rgba32Float,
            PyTextureFormat::Depth32Float => TextureFormat::Depth32Float,
            PyTextureFormat::Depth24Plus => TextureFormat::Depth24Plus,
            PyTextureFormat::Depth24PlusStencil8 => TextureFormat::Depth24PlusStencil8,
            PyTextureFormat::Bc1RgbaUnorm => TextureFormat::Bc1RgbaUnorm,
            PyTextureFormat::Bc1RgbaUnormSrgb => TextureFormat::Bc1RgbaUnormSrgb,
            PyTextureFormat::Bc2RgbaUnorm => TextureFormat::Bc2RgbaUnorm,
            PyTextureFormat::Bc2RgbaUnormSrgb => TextureFormat::Bc2RgbaUnormSrgb,
            PyTextureFormat::Bc3RgbaUnorm => TextureFormat::Bc3RgbaUnorm,
            PyTextureFormat::Bc3RgbaUnormSrgb => TextureFormat::Bc3RgbaUnormSrgb,
            PyTextureFormat::Bc4RUnorm => TextureFormat::Bc4RUnorm,
            PyTextureFormat::Bc4RSnorm => TextureFormat::Bc4RSnorm,
            PyTextureFormat::Bc5RgUnorm => TextureFormat::Bc5RgUnorm,
            PyTextureFormat::Bc5RgSnorm => TextureFormat::Bc5RgSnorm,
            PyTextureFormat::Bc6hRgbUfloat => TextureFormat::Bc6hRgbUfloat,
            PyTextureFormat::Bc6hRgbFloat => TextureFormat::Bc6hRgbFloat,
            PyTextureFormat::Bc7RgbaUnorm => TextureFormat::Bc7RgbaUnorm,
            PyTextureFormat::Bc7RgbaUnormSrgb => TextureFormat::Bc7RgbaUnormSrgb,
        }
    }
}

impl From<TextureFormat> for PyTextureFormat {
    fn from(format: TextureFormat) -> Self {
        match format {
            TextureFormat::R8Unorm => PyTextureFormat::R8Unorm,
            TextureFormat::R8Snorm => PyTextureFormat::R8Snorm,
            TextureFormat::R8Uint => PyTextureFormat::R8Uint,
            TextureFormat::R8Sint => PyTextureFormat::R8Sint,
            TextureFormat::R16Uint => PyTextureFormat::R16Uint,
            TextureFormat::R16Sint => PyTextureFormat::R16Sint,
            TextureFormat::R16Float => PyTextureFormat::R16Float,
            TextureFormat::Rg8Unorm => PyTextureFormat::Rg8Unorm,
            TextureFormat::Rg8Snorm => PyTextureFormat::Rg8Snorm,
            TextureFormat::Rg8Uint => PyTextureFormat::Rg8Uint,
            TextureFormat::Rg8Sint => PyTextureFormat::Rg8Sint,
            TextureFormat::R32Uint => PyTextureFormat::R32Uint,
            TextureFormat::R32Sint => PyTextureFormat::R32Sint,
            TextureFormat::R32Float => PyTextureFormat::R32Float,
            TextureFormat::Rg16Uint => PyTextureFormat::Rg16Uint,
            TextureFormat::Rg16Sint => PyTextureFormat::Rg16Sint,
            TextureFormat::Rg16Float => PyTextureFormat::Rg16Float,
            TextureFormat::Rgba8Unorm => PyTextureFormat::Rgba8Unorm,
            TextureFormat::Rgba8UnormSrgb => PyTextureFormat::Rgba8UnormSrgb,
            TextureFormat::Rgba8Snorm => PyTextureFormat::Rgba8Snorm,
            TextureFormat::Rgba8Uint => PyTextureFormat::Rgba8Uint,
            TextureFormat::Rgba8Sint => PyTextureFormat::Rgba8Sint,
            TextureFormat::Bgra8Unorm => PyTextureFormat::Bgra8Unorm,
            TextureFormat::Bgra8UnormSrgb => PyTextureFormat::Bgra8UnormSrgb,
            TextureFormat::Rgb10a2Unorm => PyTextureFormat::Rgb10a2Unorm,
            TextureFormat::Rg11b10Ufloat => PyTextureFormat::Rg11b10Ufloat,
            TextureFormat::Rg32Uint => PyTextureFormat::Rg32Uint,
            TextureFormat::Rg32Sint => PyTextureFormat::Rg32Sint,
            TextureFormat::Rg32Float => PyTextureFormat::Rg32Float,
            TextureFormat::Rgba16Uint => PyTextureFormat::Rgba16Uint,
            TextureFormat::Rgba16Sint => PyTextureFormat::Rgba16Sint,
            TextureFormat::Rgba16Float => PyTextureFormat::Rgba16Float,
            TextureFormat::Rgba32Uint => PyTextureFormat::Rgba32Uint,
            TextureFormat::Rgba32Sint => PyTextureFormat::Rgba32Sint,
            TextureFormat::Rgba32Float => PyTextureFormat::Rgba32Float,
            TextureFormat::Depth32Float => PyTextureFormat::Depth32Float,
            TextureFormat::Depth24Plus => PyTextureFormat::Depth24Plus,
            TextureFormat::Depth24PlusStencil8 => PyTextureFormat::Depth24PlusStencil8,
            TextureFormat::Bc1RgbaUnorm => PyTextureFormat::Bc1RgbaUnorm,
            TextureFormat::Bc1RgbaUnormSrgb => PyTextureFormat::Bc1RgbaUnormSrgb,
            TextureFormat::Bc2RgbaUnorm => PyTextureFormat::Bc2RgbaUnorm,
            TextureFormat::Bc2RgbaUnormSrgb => PyTextureFormat::Bc2RgbaUnormSrgb,
            TextureFormat::Bc3RgbaUnorm => PyTextureFormat::Bc3RgbaUnorm,
            TextureFormat::Bc3RgbaUnormSrgb => PyTextureFormat::Bc3RgbaUnormSrgb,
            TextureFormat::Bc4RUnorm => PyTextureFormat::Bc4RUnorm,
            TextureFormat::Bc4RSnorm => PyTextureFormat::Bc4RSnorm,
            TextureFormat::Bc5RgUnorm => PyTextureFormat::Bc5RgUnorm,
            TextureFormat::Bc5RgSnorm => PyTextureFormat::Bc5RgSnorm,
            TextureFormat::Bc6hRgbUfloat => PyTextureFormat::Bc6hRgbUfloat,
            TextureFormat::Bc6hRgbFloat => PyTextureFormat::Bc6hRgbFloat,
            TextureFormat::Bc7RgbaUnorm => PyTextureFormat::Bc7RgbaUnorm,
            TextureFormat::Bc7RgbaUnormSrgb => PyTextureFormat::Bc7RgbaUnormSrgb,
            // Handle formats not in our enum by defaulting to a reasonable fallback
            _ => PyTextureFormat::Rgba8Unorm,
        }
    }
}
