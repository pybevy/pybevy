use bevy::{app::App, camera::CameraPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(CameraPlugin)]
#[pyclass(name = "CameraPlugin", module = "pybevy.camera", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyCameraPlugin;

#[pymethods]
impl PyCameraPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyCameraPlugin, PyPlugin).into()
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
