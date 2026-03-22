use pybevy_core::PyPlugin;
use pyo3::prelude::*;

use crate::winit_settings::PyWinitSettings;

#[pyclass(name = "WinitPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone)]
pub struct PyWinitPlugin {
    pub settings: Option<PyWinitSettings>,
}

#[pymethods]
impl PyWinitPlugin {
    #[new]
    #[pyo3(signature = (settings = None))]
    pub fn new(settings: Option<PyWinitSettings>) -> (Self, PyPlugin) {
        (PyWinitPlugin { settings }, PyPlugin)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match &self.settings {
            Some(s) => Ok(format!("WinitPlugin(settings={})", s.__repr__())),
            None => Ok("WinitPlugin()".to_string()),
        }
    }
}

impl Default for PyWinitPlugin {
    fn default() -> Self {
        PyWinitPlugin { settings: None }
    }
}
