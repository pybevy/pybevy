use bevy::app::App;
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

use crate::winit_settings::PyWinitSettings;

#[pyplugin(bevy::winit::WinitPlugin)]
#[pyclass(name = "WinitPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Default)]
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

impl PluginBuild for PyWinitPlugin {
    fn build(py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        let config: PyRef<'_, PyWinitPlugin> = py_plugin.extract()?;
        if let Some(ref settings) = config.settings {
            app.insert_resource(bevy::winit::WinitSettings::from(settings.clone()));
        }
        app.add_plugins(bevy::winit::WinitPlugin::default());
        Ok(())
    }
}
