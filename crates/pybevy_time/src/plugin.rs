use bevy::{app::App, time::TimePlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(TimePlugin)]
#[pyclass(name = "TimePlugin", module = "pybevy.time", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyTimePlugin;

#[pymethods]
impl PyTimePlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyTimePlugin, PyPlugin).into()
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
