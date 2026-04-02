use bevy::{animation::AnimationPlugin, app::App};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::plugin_storage;
use pyo3::prelude::*;

#[plugin_storage(AnimationPlugin)]
#[pyclass(name = "AnimationPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyAnimationPlugin;

#[pymethods]
impl PyAnimationPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyAnimationPlugin, PyPlugin)
    }
}

impl Default for PyAnimationPlugin {
    fn default() -> Self {
        PyAnimationPlugin
    }
}

impl PluginBuild for PyAnimationPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(AnimationPlugin);
        Ok(())
    }
}
