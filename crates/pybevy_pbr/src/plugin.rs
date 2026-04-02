use bevy::{app::App, pbr::PbrPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::plugin_storage;
use pyo3::prelude::*;

#[plugin_storage(PbrPlugin)]
#[pyclass(name = "PbrPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyPbrPlugin;

#[pymethods]
impl PyPbrPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyPbrPlugin, PyPlugin)
    }
}

impl Default for PyPbrPlugin {
    fn default() -> Self {
        PyPbrPlugin
    }
}

impl PluginBuild for PyPbrPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(PbrPlugin::default());
        Ok(())
    }
}
