pub mod image;
pub mod image_address_mode;
pub mod image_array_layout;
pub mod image_compare_function;
pub mod image_filter_mode;
pub mod image_format;
pub mod image_format_setting;
pub mod image_sampler_border_color;
pub mod loader_settings;
pub mod plugin;
pub mod sampler_descriptor;
pub mod texture_atlas;
pub mod texture_atlas_layout;
pub mod texture_atlas_rects;
pub mod texture_atlas_sources;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        image::{PyImage, PyRenderAssetUsages},
        image_format::PyImageFormat,
        plugin::PyImagePlugin,
        texture_atlas::PyTextureAtlas,
        texture_atlas_layout::PyTextureAtlasLayout,
        texture_atlas_sources::PyTextureAtlasSources,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "image")?;
    m.add_class::<plugin::PyImagePlugin>()?;
    m.add_class::<image::PyImage>()?;
    m.add_class::<image::PyRenderAssetUsages>()?;
    m.add_class::<image::ImageDataContext>()?;
    m.add_class::<image::ImageDataContextMut>()?;
    m.add_class::<image::ImagePixelContextMut>()?;
    m.add_class::<image_compare_function::PyImageCompareFunction>()?;
    m.add_class::<image_format::PyImageFormat>()?;
    m.add_class::<image_format_setting::PyImageFormatSetting>()?;
    m.add_class::<image_sampler_border_color::PyImageSamplerBorderColor>()?;
    m.add_class::<sampler_descriptor::PyImageSamplerDescriptor>()?;
    m.add_class::<loader_settings::PyImageSampler>()?;
    m.add_class::<loader_settings::PyImageLoaderSettings>()?;
    m.add_class::<image_address_mode::PyImageAddressMode>()?;
    m.add_class::<image_filter_mode::PyImageFilterMode>()?;
    m.add_class::<image_array_layout::PyImageArrayLayout>()?;
    m.add_class::<texture_atlas::PyTextureAtlas>()?;
    m.add_class::<texture_atlas_layout::PyTextureAtlasLayout>()?;
    m.add_class::<texture_atlas_rects::PyTextureAtlasRects>()?;
    m.add_class::<texture_atlas_sources::PyTextureAtlasSources>()?;
    loader_settings::register_image_sampler_variants(&m)?;
    parent.add_submodule(&m)
}
