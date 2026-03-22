use pybevy_core::PyPlugin;
use pyo3::prelude::*;
#[pyclass(name = "CameraPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyCameraPlugin;

#[pymethods]
impl PyCameraPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyCameraPlugin, PyPlugin)
    }
}

impl Default for PyCameraPlugin {
    fn default() -> Self {
        PyCameraPlugin
    }
}
