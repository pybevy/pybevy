use bevy::app::App;
use pybevy_core::{PluginBuild, PyPlugin};
use pybevy_macros::plugin_storage;
use pyo3::prelude::*;

use crate::power_preference::PyPowerPreference;

#[plugin_storage(bevy::render::RenderPlugin)]
#[pyclass(name = "RenderPlugin", extends = PyPlugin, frozen)]
#[derive(Debug, Clone, Default)]
pub struct PyRenderPlugin {
    pub power_preference: Option<PyPowerPreference>,
    pub synchronous_pipeline_compilation: Option<bool>,
}

#[pymethods]
impl PyRenderPlugin {
    #[new]
    #[pyo3(signature = (power_preference = None, synchronous_pipeline_compilation = None))]
    pub fn new(
        power_preference: Option<PyPowerPreference>,
        synchronous_pipeline_compilation: Option<bool>,
    ) -> (Self, PyPlugin) {
        (
            PyRenderPlugin {
                power_preference,
                synchronous_pipeline_compilation,
            },
            PyPlugin,
        )
    }

    pub fn __repr__(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref pp) = self.power_preference {
            parts.push(format!("power_preference={pp:?}"));
        }
        if let Some(sync) = self.synchronous_pipeline_compilation {
            parts.push(format!("synchronous_pipeline_compilation={sync}"));
        }
        if parts.is_empty() {
            "RenderPlugin()".to_string()
        } else {
            format!("RenderPlugin({})", parts.join(", "))
        }
    }
}

impl PluginBuild for PyRenderPlugin {
    fn build(py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        let config: PyRef<'_, PyRenderPlugin> = py_plugin.extract()?;
        let mut wgpu_settings = bevy::render::settings::WgpuSettings::default();
        if let Some(ref pp) = config.power_preference {
            wgpu_settings.power_preference = (*pp).into();
        }
        let mut render_plugin = bevy::render::RenderPlugin {
            render_creation: bevy::render::settings::RenderCreation::Automatic(wgpu_settings),
            ..Default::default()
        };
        if let Some(sync) = config.synchronous_pipeline_compilation {
            render_plugin.synchronous_pipeline_compilation = sync;
        }
        app.add_plugins(render_plugin);
        Ok(())
    }
}
