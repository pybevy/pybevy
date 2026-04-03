use bevy::{app::App, text::TextPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(TextPlugin)]
#[pyclass(name = "TextPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyTextPlugin;

#[pymethods]
impl PyTextPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyTextPlugin, PyPlugin)
    }
}

impl Default for PyTextPlugin {
    fn default() -> Self {
        PyTextPlugin
    }
}

impl PluginBuild for PyTextPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(TextPlugin);
        Ok(())
    }
}
