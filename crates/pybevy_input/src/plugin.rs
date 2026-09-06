use bevy::{app::App, input::InputPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(InputPlugin)]
#[pyclass(name = "InputPlugin", module = "pybevy.input", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyInputPlugin;

#[pymethods]
impl PyInputPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyInputPlugin, PyPlugin).into()
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
