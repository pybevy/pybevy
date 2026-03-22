use pyo3::prelude::*;

#[pyclass(name = "Camera3dDepthTextureUsage", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyCamera3dDepthTextureUsage(pub u32);

#[pymethods]
impl PyCamera3dDepthTextureUsage {
    #[new]
    pub fn new(value: u32) -> Self {
        PyCamera3dDepthTextureUsage(value)
    }

    #[getter]
    pub fn value(&self) -> u32 {
        self.0
    }

    pub fn __repr__(&self) -> String {
        format!("Camera3dDepthTextureUsage({})", self.0)
    }
}
