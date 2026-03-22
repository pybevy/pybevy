use pybevy_core::PyPlugin;
use pyo3::prelude::*;
#[pyclass(name = "CorePipelinePlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyCorePipelinePlugin;

#[pymethods]
impl PyCorePipelinePlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyCorePipelinePlugin, PyPlugin)
    }
}

impl Default for PyCorePipelinePlugin {
    fn default() -> Self {
        PyCorePipelinePlugin
    }
}
