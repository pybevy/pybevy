use pybevy_core::PyPlugin;
use pyo3::prelude::*;
#[pyclass(name = "GltfPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyGltfPlugin;

#[pymethods]
impl PyGltfPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyGltfPlugin, PyPlugin)
    }
}

impl Default for PyGltfPlugin {
    fn default() -> Self {
        PyGltfPlugin
    }
}
