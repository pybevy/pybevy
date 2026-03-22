use pybevy_core::PyPlugin;
use pyo3::prelude::*;

#[pyclass(name = "TimePlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyTimePlugin;

#[pymethods]
impl PyTimePlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyTimePlugin, PyPlugin)
    }
}

impl Default for PyTimePlugin {
    fn default() -> Self {
        PyTimePlugin
    }
}
