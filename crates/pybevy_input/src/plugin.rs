use pybevy_core::PyPlugin;
use pyo3::prelude::*;
#[pyclass(name = "InputPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyInputPlugin;

#[pymethods]
impl PyInputPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyInputPlugin, PyPlugin)
    }
}

impl Default for PyInputPlugin {
    fn default() -> Self {
        PyInputPlugin
    }
}
