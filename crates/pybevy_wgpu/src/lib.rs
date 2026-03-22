pub mod extent3d;
pub mod texture_dimension;
pub mod texture_format;

pub use extent3d::PyExtent3d;
use pyo3::prelude::*;
pub use texture_dimension::PyTextureDimension;
pub use texture_format::PyTextureFormat;

pub fn add_wgpu_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyExtent3d>()?;
    m.add_class::<PyTextureDimension>()?;
    m.add_class::<PyTextureFormat>()?;
    Ok(())
}
