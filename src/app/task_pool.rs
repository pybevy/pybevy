use bevy::app::TaskPoolPlugin;
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

use crate::app::app::PyApp;

#[pyplugin(TaskPoolPlugin, default_plugin = TaskPool)]
#[pyclass(name = "TaskPoolPlugin", module = "pybevy.app", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyTaskPoolPlugin;

#[pymethods]
impl PyTaskPoolPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyTaskPoolPlugin, PyPlugin).into()
    }

    pub fn build(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        app.borrow().with_bevy_app(|bevy_app| {
            bevy_app.add_plugins(TaskPoolPlugin::default());
            Ok(())
        })
    }
}

impl PluginBuild for PyTaskPoolPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut bevy::app::App) -> PyResult<()> {
        app.add_plugins(TaskPoolPlugin::default());
        Ok(())
    }
}
