use bevy::{app::App, image::ImagePlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(ImagePlugin)]
#[pyclass(name = "ImagePlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyImagePlugin;

#[pymethods]
impl PyImagePlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyImagePlugin, PyPlugin)
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
