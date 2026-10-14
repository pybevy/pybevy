pub mod auto_focus;
pub mod focus_cause;
pub mod input_focus;
pub mod plugin;
pub mod tab_navigation;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        auto_focus::PyAutoFocus,
        focus_cause::PyFocusCause,
        input_focus::{PyInputFocus, PyInputFocusVisible},
        plugin::{PyInputDispatchPlugin, PyInputFocusPlugin, PyTabNavigationPlugin},
        tab_navigation::{PyTabGroup, PyTabIndex},
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "input_focus")?;
    m.add_class::<auto_focus::PyAutoFocus>()?;
    m.add_class::<focus_cause::PyFocusCause>()?;
    m.add_class::<input_focus::PyInputFocus>()?;
    m.add_class::<input_focus::PyInputFocusVisible>()?;
    m.add_class::<plugin::PyInputDispatchPlugin>()?;
    m.add_class::<plugin::PyInputFocusPlugin>()?;
    m.add_class::<plugin::PyTabNavigationPlugin>()?;
    m.add_class::<tab_navigation::PyTabGroup>()?;
    m.add_class::<tab_navigation::PyTabIndex>()?;
    parent.add_submodule(&m)
}
