use bevy::window::WindowPlugin;
use pybevy_core::PyPlugin;
use pyo3::prelude::*;

use crate::{PyExitCondition, PyWindow};

#[pyclass(name = "WindowPlugin", extends = PyPlugin)]
pub struct PyWindowPlugin {
    primary_window: Option<PyWindow>,
    exit_condition: Option<PyExitCondition>,
}

#[pymethods]
impl PyWindowPlugin {
    #[new]
    #[pyo3(signature = (primary_window = None, exit_condition = None))]
    pub fn new(
        primary_window: Option<PyRef<'_, PyWindow>>,
        exit_condition: Option<PyExitCondition>,
    ) -> (Self, PyPlugin) {
        (
            PyWindowPlugin {
                primary_window: primary_window.map(|w| (*w).clone()),
                exit_condition,
            },
            PyPlugin,
        )
    }
}

impl TryFrom<&PyWindowPlugin> for WindowPlugin {
    type Error = PyErr;

    fn try_from(py_plugin: &PyWindowPlugin) -> PyResult<Self> {
        let primary_window = if let Some(ref py_window) = py_plugin.primary_window {
            Some(py_window.clone().try_into()?)
        } else {
            None
        };

        let mut plugin = WindowPlugin {
            primary_window,
            ..Default::default()
        };

        if let Some(exit_condition) = py_plugin.exit_condition {
            plugin.exit_condition = exit_condition.into();
        }

        Ok(plugin)
    }
}
