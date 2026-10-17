use pyo3::{exceptions::PyValueError, prelude::*};

use crate::texture_usage_rules::check_depth_texture_usage;

#[pyclass(
    name = "Camera3dDepthTextureUsage",
    module = "pybevy.camera",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyCamera3dDepthTextureUsage(pub u32);

#[pymethods]
impl PyCamera3dDepthTextureUsage {
    #[new]
    pub fn new(value: u32) -> PyResult<Self> {
        check_depth_texture_usage(value).map_err(PyValueError::new_err)?;
        Ok(PyCamera3dDepthTextureUsage(value))
    }

    #[getter]
    pub fn value(&self) -> u32 {
        self.0
    }

    pub fn __repr__(&self) -> String {
        format!("Camera3dDepthTextureUsage({})", self.0)
    }
}
