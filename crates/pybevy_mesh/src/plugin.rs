use bevy::{app::App, mesh::MeshPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::plugin_storage;
use pyo3::prelude::*;

#[plugin_storage(MeshPlugin)]
#[pyclass(name = "MeshPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyMeshPlugin;

#[pymethods]
impl PyMeshPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyMeshPlugin, PyPlugin)
    }
}

impl Default for PyMeshPlugin {
    fn default() -> Self {
        PyMeshPlugin
    }
}

impl PluginBuild for PyMeshPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(MeshPlugin);
        Ok(())
    }
}
