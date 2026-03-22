pub mod accessibility_node;
pub mod role;

pub use accessibility_node::PyAccessibilityNode;
use bevy::{
    a11y::AccessibilityNode,
    app::{App, Plugin},
};
use pybevy_core::{
    DynamicComponentRegistry, PyPlugin, plugin::plugin_registry, registry::global_registry,
};
use pybevy_macros::{component_bridge, plugin_bridge};
use pyo3::prelude::*;
pub use role::PyRole;

component_bridge!(AccessibilityNode, PyAccessibilityNode);
plugin_bridge!(PyAccessibilityPlugin, bevy::a11y::AccessibilityPlugin);

#[pyclass(name = "AccessibilityPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyAccessibilityPlugin;

#[pymethods]
impl PyAccessibilityPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyAccessibilityPlugin, PyPlugin)
    }
}

impl Default for PyAccessibilityPlugin {
    fn default() -> Self {
        PyAccessibilityPlugin
    }
}

pub struct PyBevyA11yPlugin;

impl Plugin for PyBevyA11yPlugin {
    fn build(&self, app: &mut App) {
        global_registry::register_component_bridge(AccessibilityNodeBridge);

        if let Some(mut registry) = app
            .world_mut()
            .get_resource_mut::<DynamicComponentRegistry>()
        {
            registry.register(AccessibilityNodeBridge);
        }
    }
}

pub fn register_a11y_bridges() {
    global_registry::register_component_bridge(AccessibilityNodeBridge);
    plugin_registry::register_plugin_bridge(AccessibilityPluginBridge);
}

pub fn add_a11y_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_a11y_bridges();
    m.add_class::<PyAccessibilityPlugin>()?;
    m.add_class::<PyAccessibilityNode>()?;
    m.add_class::<PyRole>()?;
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "a11y")?;
    add_a11y_classes(&m)?;
    parent.add_submodule(&m)
}
