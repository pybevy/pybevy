use pybevy_core::PyPlugin;
use pyo3::prelude::*;

#[pyclass(name = "TextPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyTextPlugin;

#[pymethods]
impl PyTextPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyTextPlugin, PyPlugin)
    }
}

impl Default for PyTextPlugin {
    fn default() -> Self {
        PyTextPlugin
    }
}
