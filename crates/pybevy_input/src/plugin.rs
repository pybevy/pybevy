use bevy::{app::App, input::InputPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::plugin_storage;
use pyo3::prelude::*;

#[plugin_storage(InputPlugin)]
#[pyclass(name = "InputPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyInputPlugin;

#[pymethods]
impl PyInputPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyInputPlugin, PyPlugin)
    }
}

impl Default for PyInputPlugin {
    fn default() -> Self {
        PyInputPlugin
    }
}

impl PluginBuild for PyInputPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(InputPlugin);
        Ok(())
    }
}
