use bevy::{
    app::App,
    input_focus::{InputDispatchPlugin, InputFocusPlugin, tab_navigation::TabNavigationPlugin},
};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(InputFocusPlugin)]
#[pyclass(name = "InputFocusPlugin", module = "pybevy.input_focus", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PyInputFocusPlugin;

#[pymethods]
impl PyInputFocusPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyInputFocusPlugin, PyPlugin).into()
    }
}

impl PluginBuild for PyInputFocusPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(InputFocusPlugin);
        Ok(())
    }
}

#[pyplugin(InputDispatchPlugin)]
#[pyclass(name = "InputDispatchPlugin", module = "pybevy.input_focus", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PyInputDispatchPlugin;

#[pymethods]
impl PyInputDispatchPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyInputDispatchPlugin, PyPlugin).into()
    }
}

impl PluginBuild for PyInputDispatchPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(InputDispatchPlugin);
        Ok(())
    }
}

#[pyplugin(TabNavigationPlugin)]
#[pyclass(name = "TabNavigationPlugin", module = "pybevy.input_focus", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PyTabNavigationPlugin;

#[pymethods]
impl PyTabNavigationPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyTabNavigationPlugin, PyPlugin).into()
    }
}

impl PluginBuild for PyTabNavigationPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(TabNavigationPlugin);
        Ok(())
    }
}
