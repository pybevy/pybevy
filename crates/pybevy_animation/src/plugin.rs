use pybevy_core::PyPlugin;
use pyo3::prelude::*;

#[pyclass(name = "AnimationPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyAnimationPlugin;

#[pymethods]
impl PyAnimationPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyAnimationPlugin, PyPlugin)
    }
}

impl Default for PyAnimationPlugin {
    fn default() -> Self {
        PyAnimationPlugin
    }
}
