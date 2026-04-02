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
pub use border_rect::PyBorderRect;
pub use color_material::PyColorMaterial;
pub use plugin::{PyColorMaterialPlugin, PySpritePlugin};
use pyo3::prelude::*;
pub use scaling_mode::PySpriteScalingMode;
pub use slice_scale_mode::PySliceScaleMode;
pub use sprite::PySprite;
pub use sprite_image_mode::PySpriteImageMode;
pub use texture_slicer::PyTextureSlicer;

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "sprite")?;
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
    m.add_class::<PyTextureSlicer>()?;
    parent.add_submodule(&m)
}
