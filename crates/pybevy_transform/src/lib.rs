pub mod global_transform;
pub mod transform;

use bevy::{app::App, transform::TransformPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::plugin_storage;
use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        PyTransformPlugin, global_transform::PyGlobalTransform, transform::PyTransform,
    };
}

#[plugin_storage(TransformPlugin)]
#[pyclass(name = "TransformPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyTransformPlugin;

#[pymethods]
impl PyTransformPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyTransformPlugin, PyPlugin)
    }
}

impl Default for PyTransformPlugin {
    fn default() -> Self {
        PyTransformPlugin
    }
}

impl PluginBuild for PyTransformPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(TransformPlugin);
        Ok(())
    }
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "transform")?;
    m.add_class::<PyTransformPlugin>()?;
    m.add_class::<transform::PyTransform>()?;
    m.add_class::<global_transform::PyGlobalTransform>()?;
    parent.add_submodule(&m)
}
