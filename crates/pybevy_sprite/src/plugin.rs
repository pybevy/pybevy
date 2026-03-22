use pybevy_core::PyPlugin;
use pyo3::prelude::*;

#[pyclass(name = "SpritePlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PySpritePlugin;

#[pymethods]
impl PySpritePlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PySpritePlugin, PyPlugin)
    }
}

impl Default for PySpritePlugin {
    fn default() -> Self {
        PySpritePlugin
    }
}

#[pyclass(name = "ColorMaterialPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyColorMaterialPlugin;

#[pymethods]
impl PyColorMaterialPlugin {
    #[new]
    pub fn new() -> (Self, PyPlugin) {
        (PyColorMaterialPlugin, PyPlugin)
    }
}

impl Default for PyColorMaterialPlugin {
    fn default() -> Self {
        PyColorMaterialPlugin
    }
}
