use bevy::{app::App, gizmos::GizmoPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(GizmoPlugin)]
#[pyclass(name = "GizmoPlugin", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyGizmoPlugin;

#[pymethods]
impl PyGizmoPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyGizmoPlugin, PyPlugin).into()
    }
}

impl Default for PyGizmoPlugin {
    fn default() -> Self {
        PyGizmoPlugin
    }
}

impl PluginBuild for PyGizmoPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(GizmoPlugin);
        Ok(())
    }
}
