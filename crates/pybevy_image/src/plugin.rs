use pybevy_core::PyPlugin;
use pyo3::prelude::*;
#[pyclass(name = "ImagePlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyImagePlugin;

#[pymethods]
impl PyImagePlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyImagePlugin, PyPlugin)
    }
}

impl Default for PyImagePlugin {
    fn default() -> Self {
        PyImagePlugin
    }
}
