use bevy::{app::App, window::WindowPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

use crate::{exit_condition::PyExitCondition, window::PyWindow};

#[pyplugin(bevy::window::WindowPlugin)]
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
    ) -> PyResult<(Self, PyPlugin)> {
        let window = match primary_window {
            Some(w) => Some(w.storage.as_ref()?.clone().into()),
            None => None,
        };
        Ok((
            PyWindowPlugin {
                primary_window: window,
                exit_condition,
            },
            PyPlugin,
        ))
    }
}

impl PluginBuild for PyWindowPlugin {
    fn build(py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        let config: PyRef<'_, PyWindowPlugin> = py_plugin.extract()?;
        app.add_plugins(WindowPlugin::try_from(&*config)?);
        Ok(())
    }
}

impl TryFrom<&PyWindowPlugin> for WindowPlugin {
    type Error = PyErr;

    fn try_from(py_plugin: &PyWindowPlugin) -> PyResult<Self> {
        let primary_window = if let Some(ref py_window) = py_plugin.primary_window {
            Some(py_window.as_ref()?.clone())
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
