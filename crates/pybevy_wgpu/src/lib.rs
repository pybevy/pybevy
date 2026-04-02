pub mod extent3d;
pub mod texture_dimension;
pub mod texture_format;

use pyo3::prelude::*;

pub use extent3d::PyExtent3d;
pub use texture_dimension::PyTextureDimension;
pub use texture_format::PyTextureFormat;

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "wgpu")?;
    m.add_class::<PyExtent3d>()?;
    m.add_class::<PyTextureDimension>()?;
    m.add_class::<PyTextureFormat>()?;
    parent.add_submodule(&m)
}
