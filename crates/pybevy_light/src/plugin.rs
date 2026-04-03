use bevy::{app::App, light::LightPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(LightPlugin)]
#[pyclass(name = "LightPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyLightPlugin;

#[pymethods]
impl PyLightPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyLightPlugin, PyPlugin)
    }
}

impl Default for PyLightPlugin {
    fn default() -> Self {
        PyLightPlugin
    }
}

impl PluginBuild for PyLightPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(LightPlugin);
        Ok(())
    }
}
