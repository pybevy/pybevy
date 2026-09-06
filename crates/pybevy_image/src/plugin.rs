use bevy::{app::App, image::ImagePlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(ImagePlugin, default_plugin = Image)]
#[pyclass(name = "ImagePlugin", module = "pybevy.image", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyImagePlugin;

#[pymethods]
impl PyImagePlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyImagePlugin, PyPlugin).into()
    }
}

impl Default for PyImagePlugin {
    fn default() -> Self {
        PyImagePlugin
    }
}

impl PluginBuild for PyImagePlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(ImagePlugin::default());
        Ok(())
    }
}
