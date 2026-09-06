use bevy::{app::App, sprite::SpritePlugin, sprite_render::ColorMaterialPlugin};
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

#[pyplugin(SpritePlugin)]
#[pyclass(name = "SpritePlugin", module = "pybevy.sprite", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PySpritePlugin;

#[pymethods]
impl PySpritePlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PySpritePlugin, PyPlugin).into()
    }
}

impl Default for PySpritePlugin {
    fn default() -> Self {
        PySpritePlugin
    }
}

impl PluginBuild for PySpritePlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(SpritePlugin);
        Ok(())
    }
}

#[pyplugin(ColorMaterialPlugin)]
#[pyclass(name = "ColorMaterialPlugin", module = "pybevy.sprite", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyColorMaterialPlugin;

#[pymethods]
impl PyColorMaterialPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyColorMaterialPlugin, PyPlugin).into()
    }
}

impl Default for PyColorMaterialPlugin {
    fn default() -> Self {
        PyColorMaterialPlugin
    }
}

impl PluginBuild for PyColorMaterialPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(ColorMaterialPlugin);
        Ok(())
    }
}
