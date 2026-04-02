use bevy::{app::App, camera::CameraPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::plugin_storage;
use pyo3::prelude::*;

#[plugin_storage(CameraPlugin)]
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

impl PluginBuild for PyCameraPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(CameraPlugin);
        Ok(())
    }
}
