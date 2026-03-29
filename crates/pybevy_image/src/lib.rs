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
pub mod texture_atlas_sources;

use bevy::image::TextureAtlasLayout;
pub use image::{
    ImageDataContext, ImageDataContextMut, ImagePixelContextMut, PyImage, PyRenderAssetUsages,
};
pub use image_address_mode::PyImageAddressMode;
pub use image_array_layout::PyImageArrayLayout;
pub use image_compare_function::PyImageCompareFunction;
pub use image_filter_mode::PyImageFilterMode;
pub use image_format::PyImageFormat;
pub use image_format_setting::{PyImageFormatSetting, PyImageFormatSettingWithFormat};
pub use image_sampler_border_color::PyImageSamplerBorderColor;
pub use loader_settings::{PyImageLoaderSettings, PyImageSampler};
pub use plugin::PyImagePlugin;
use pybevy_core::{plugin::plugin_registry, registry::global_registry};
use pybevy_macros::{asset_bridge, plugin_bridge};
use pyo3::prelude::*;
pub use sampler_descriptor::PyImageSamplerDescriptor;
pub use texture_atlas::PyTextureAtlas;
pub use texture_atlas_layout::PyTextureAtlasLayout;
pub use texture_atlas_sources::PyTextureAtlasSources;

plugin_bridge!(PyImagePlugin, bevy::image::ImagePlugin);

asset_bridge!(TextureAtlasLayout, PyTextureAtlasLayout, not_loadable);
pub fn register_image_bridges() {
    plugin_registry::register_plugin_bridge(ImagePluginBridge);
    global_registry::register_asset_bridge(TextureAtlasLayoutBridge);
}
pub fn add_image_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_image_bridges();

    m.add_class::<PyImagePlugin>()?;
    m.add_class::<PyImage>()?;
    m.add_class::<PyRenderAssetUsages>()?;
    m.add_class::<ImageDataContext>()?;
    m.add_class::<ImageDataContextMut>()?;
    m.add_class::<ImagePixelContextMut>()?;
    m.add_class::<PyImageCompareFunction>()?;
    m.add_class::<PyImageFormat>()?;
    m.add_class::<PyImageFormatSetting>()?;
    m.add_class::<PyImageFormatSettingWithFormat>()?;
    m.add_class::<PyImageSamplerBorderColor>()?;
    m.add_class::<PyImageSamplerDescriptor>()?;
    m.add_class::<PyImageSampler>()?;
    m.add_class::<PyImageLoaderSettings>()?;
    m.add_class::<PyImageAddressMode>()?;
    m.add_class::<PyImageFilterMode>()?;
    m.add_class::<PyImageArrayLayout>()?;
    m.add_class::<PyTextureAtlas>()?;
    m.add_class::<PyTextureAtlasLayout>()?;
    m.add_class::<PyTextureAtlasSources>()?;

    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "image")?;
    add_image_classes(&m)?;
    parent.add_submodule(&m)
}
