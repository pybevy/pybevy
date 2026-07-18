pub mod extent3d;
pub mod texture_dimension;
pub mod texture_format;
pub mod vertex_format;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        extent3d::PyExtent3d, texture_dimension::PyTextureDimension,
        texture_format::PyTextureFormat, vertex_format::PyVertexFormat,
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "wgpu")?;
    m.add_class::<extent3d::PyExtent3d>()?;
    m.add_class::<texture_dimension::PyTextureDimension>()?;
    m.add_class::<texture_format::PyTextureFormat>()?;
    m.add_class::<vertex_format::PyVertexFormat>()?;
    parent.add_submodule(&m)
}
