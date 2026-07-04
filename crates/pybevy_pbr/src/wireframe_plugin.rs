use bevy::{app::App, pbr::wireframe::WireframePlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(WireframePlugin)]
#[pyclass(name = "WireframePlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyWireframePlugin;

#[pymethods]
impl PyWireframePlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyWireframePlugin, PyPlugin)
    }
}

impl Default for PyWireframePlugin {
    fn default() -> Self {
        PyWireframePlugin
    }
}

impl PluginBuild for PyWireframePlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(WireframePlugin::default());
        Ok(())
    }
}
