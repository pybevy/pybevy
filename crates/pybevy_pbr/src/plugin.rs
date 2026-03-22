use pybevy_core::PyPlugin;
use pyo3::prelude::*;
#[pyclass(name = "PbrPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyPbrPlugin;

#[pymethods]
impl PyPbrPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyPbrPlugin, PyPlugin)
    }
}

impl Default for PyPbrPlugin {
    fn default() -> Self {
        PyPbrPlugin
    }
}
