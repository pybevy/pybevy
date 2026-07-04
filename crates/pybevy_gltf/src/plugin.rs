use bevy::{app::App, gltf::GltfPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(GltfPlugin)]
#[pyclass(name = "GltfPlugin", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyGltfPlugin;

#[pymethods]
impl PyGltfPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyGltfPlugin, PyPlugin).into()
    }
}

impl Default for PyGltfPlugin {
    fn default() -> Self {
        PyGltfPlugin
    }
}

impl PluginBuild for PyGltfPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(GltfPlugin::default());
        Ok(())
    }
}
