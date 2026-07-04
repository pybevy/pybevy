use bevy::{app::App, mesh::MeshPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(MeshPlugin)]
#[pyclass(name = "MeshPlugin", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyMeshPlugin;

#[pymethods]
impl PyMeshPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyMeshPlugin, PyPlugin).into()
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
