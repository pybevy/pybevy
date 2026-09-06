use bevy::{app::App, pbr::PbrPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(PbrPlugin)]
#[pyclass(name = "PbrPlugin", module = "pybevy.pbr", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyPbrPlugin;

#[pymethods]
impl PyPbrPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyPbrPlugin, PyPlugin).into()
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
