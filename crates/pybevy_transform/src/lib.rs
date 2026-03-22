pub mod global_transform;
pub mod transform;

use bevy::transform::components::{GlobalTransform, Transform};
pub use global_transform::PyGlobalTransform;
use pybevy_core::{PyPlugin, plugin::plugin_registry, registry::global_registry};
use pybevy_macros::{component_bridge, plugin_bridge};
use pyo3::prelude::*;
pub use transform::PyTransform;

component_bridge!(
    Transform,
    PyTransform,
    view_fields = [translation, rotation, scale]
);
component_bridge!(GlobalTransform, PyGlobalTransform, no_insert);

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

plugin_bridge!(PyTransformPlugin, bevy::transform::TransformPlugin);

pub fn register_transform_bridges() {
    global_registry::register_component_bridge(TransformBridge);
    global_registry::register_component_bridge(GlobalTransformBridge);
    plugin_registry::register_plugin_bridge(TransformPluginBridge);
    register_transform_batch();
}

pub fn add_transform_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_transform_bridges();
    m.add_class::<PyTransformPlugin>()?;
    m.add_class::<PyTransform>()?;
    m.add_class::<PyGlobalTransform>()?;
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "transform")?;
    add_transform_classes(&m)?;
    parent.add_submodule(&m)
}
