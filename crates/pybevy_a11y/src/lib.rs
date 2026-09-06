pub mod accessibility_node;
pub mod role;

use bevy::{a11y::AccessibilityPlugin, app::App};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{PyAccessibilityPlugin, accessibility_node::PyAccessibilityNode, role::PyRole};
}

#[pyplugin(AccessibilityPlugin)]
#[pyclass(name = "AccessibilityPlugin", module = "pybevy.a11y", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyAccessibilityPlugin;

#[pymethods]
impl PyAccessibilityPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyAccessibilityPlugin, PyPlugin).into()
    }
}

impl Default for PyAccessibilityPlugin {
    fn default() -> Self {
        PyAccessibilityPlugin
    }
}

impl PluginBuild for PyAccessibilityPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(AccessibilityPlugin);
        Ok(())
    }
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "a11y")?;
    m.add_class::<PyAccessibilityPlugin>()?;
    m.add_class::<accessibility_node::PyAccessibilityNode>()?;
    m.add_class::<role::PyRole>()?;
    parent.add_submodule(&m)
}
