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

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        border_rect::PyBorderRect,
        color_material::PyColorMaterial,
        plugin::{PyColorMaterialPlugin, PySpritePlugin},
        scaling_mode::PySpriteScalingMode,
        slice_scale_mode::PySliceScaleMode,
        sprite::PySprite,
        sprite_image_mode::PySpriteImageMode,
        texture_slicer::PyTextureSlicer,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "sprite")?;
    m.add_class::<plugin::PySpritePlugin>()?;
    m.add_class::<plugin::PyColorMaterialPlugin>()?;

    m.add_class::<anchor::PyAnchor>()?;
    m.add_class::<sprite::PySprite>()?;

    m.add_class::<color_material::PyColorMaterial>()?;

    m.add_class::<alpha_mode_2d::PyAlphaMode2d>()?;
    m.add_class::<border_rect::PyBorderRect>()?;
    m.add_class::<scaling_mode::PySpriteScalingMode>()?;
    m.add_class::<slice_scale_mode::PySliceScaleMode>()?;
    m.add_class::<sprite_image_mode::PySpriteImageMode>()?;
    m.add_class::<texture_slicer::PyTextureSlicer>()?;
    parent.add_submodule(&m)
}
