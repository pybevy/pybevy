use bevy::{animation::AnimationPlugin, app::App};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(AnimationPlugin)]
#[pyclass(name = "AnimationPlugin", module = "pybevy.animation", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyAnimationPlugin;

#[pymethods]
impl PyAnimationPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyAnimationPlugin, PyPlugin).into()
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
