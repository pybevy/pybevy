use pybevy_core::PyPlugin;
use pyo3::prelude::*;
#[pyclass(name = "MeshPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyMeshPlugin;

#[pymethods]
impl PyMeshPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyMeshPlugin, PyPlugin)
    }
}

impl Default for PyMeshPlugin {
    fn default() -> Self {
        PyMeshPlugin
    }
}
