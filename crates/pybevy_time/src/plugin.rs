use bevy::{app::App, time::TimePlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::plugin_storage;
use pyo3::prelude::*;

#[plugin_storage(TimePlugin)]
#[pyclass(name = "TimePlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyTimePlugin;

#[pymethods]
impl PyTimePlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyTimePlugin, PyPlugin)
    }
}

impl Default for PyTimePlugin {
    fn default() -> Self {
        PyTimePlugin
    }
}

impl PluginBuild for PyTimePlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(TimePlugin);
        Ok(())
    }
}
