pub mod alpha_mode_2d;
pub mod anchor;
pub mod border_rect;
pub mod color_material;
pub mod plugin;
pub mod scaling_mode;
pub mod slice_scale_mode;
pub mod sprite;
pub mod sprite_image_mode;
pub mod texture_slicer;

pub use alpha_mode_2d::PyAlphaMode2d;
pub use anchor::PyAnchor;
use bevy::{
    sprite::{Anchor, Sprite},
    sprite_render::ColorMaterial,
};
pub use border_rect::PyBorderRect;
pub use color_material::PyColorMaterial;
pub use plugin::{PyColorMaterialPlugin, PySpritePlugin};
use pybevy_core::{plugin::plugin_registry, registry::global_registry};
pub use pybevy_image::PyTextureAtlas;
use pybevy_macros::{asset_bridge, component_bridge, plugin_bridge};
use pyo3::prelude::*;
pub use scaling_mode::PySpriteScalingMode;
pub use slice_scale_mode::PySliceScaleMode;
pub use sprite::PySprite;
pub use sprite_image_mode::PySpriteImageMode;
pub use texture_slicer::PyTextureSlicer;

component_bridge!(Anchor, PyAnchor);
component_bridge!(
    Sprite,
    PySprite,
    view_fields = [flip_x, flip_y],
    batch_only_fields = [color]
);

asset_bridge!(ColorMaterial, PyColorMaterial);

plugin_bridge!(PySpritePlugin, bevy::sprite::SpritePlugin);
plugin_bridge!(
    PyColorMaterialPlugin,
    bevy::sprite_render::ColorMaterialPlugin
);

pub fn register_sprite_bridges() {
    global_registry::register_component_bridge(AnchorBridge);
    global_registry::register_component_bridge(SpriteBridge);
    register_sprite_batch();
    global_registry::register_asset_bridge(ColorMaterialBridge);
    plugin_registry::register_plugin_bridge(SpritePluginBridge);
    plugin_registry::register_plugin_bridge(ColorMaterialPluginBridge);
}

pub fn add_sprite_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_sprite_bridges();

    m.add_class::<PySpritePlugin>()?;
    m.add_class::<PyColorMaterialPlugin>()?;

    m.add_class::<PyAnchor>()?;
    m.add_class::<PySprite>()?;

    m.add_class::<PyColorMaterial>()?;

    m.add_class::<PyAlphaMode2d>()?;
    m.add_class::<PyBorderRect>()?;
    m.add_class::<PySpriteScalingMode>()?;
    m.add_class::<PySliceScaleMode>()?;
    m.add_class::<PySpriteImageMode>()?;
    m.add_class::<PyTextureAtlas>()?;
    m.add_class::<PyTextureSlicer>()?;
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "sprite")?;
    add_sprite_classes(&m)?;
    parent.add_submodule(&m)
}
