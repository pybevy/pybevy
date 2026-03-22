use pybevy_core::PyPlugin;
use pyo3::prelude::*;
#[pyclass(name = "LightPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyLightPlugin;

#[pymethods]
impl PyLightPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyLightPlugin, PyPlugin)
    }
}

impl Default for PyLightPlugin {
    fn default() -> Self {
        PyLightPlugin
    }
}
