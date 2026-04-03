use bevy::{app::App, core_pipeline::CorePipelinePlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(CorePipelinePlugin)]
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

impl PluginBuild for PyCorePipelinePlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(CorePipelinePlugin);
        Ok(())
    }
}
