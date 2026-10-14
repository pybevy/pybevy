use bevy::{app::App, render::texture::TexturePlugin};
use pybevy_core::{
    PluginBuild, PyPlugin,
    repr::{ReprLiteral, object_repr},
};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

use crate::power_preference::PyPowerPreference;

#[pyplugin(TexturePlugin)]
#[pyclass(name = "TexturePlugin", module = "pybevy.render", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PyTexturePlugin;

#[pymethods]
impl PyTexturePlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (Self, PyPlugin).into()
    }
}

impl PluginBuild for PyTexturePlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(TexturePlugin);
        Ok(())
    }
}

#[pyplugin(bevy::render::RenderPlugin, default_plugin = Render)]
#[pyclass(name = "RenderPlugin", module = "pybevy.render", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Default)]
pub struct PyRenderPlugin {
    pub power_preference: Option<PyPowerPreference>,
    pub synchronous_pipeline_compilation: bool,
}

#[pymethods]
impl PyRenderPlugin {
    #[new]
    #[pyo3(signature = (*, power_preference = None, synchronous_pipeline_compilation = false))]
    pub fn new(
        power_preference: Option<PyPowerPreference>,
        synchronous_pipeline_compilation: bool,
    ) -> PyClassInitializer<Self> {
        (
            PyRenderPlugin {
                power_preference,
                synchronous_pipeline_compilation,
            },
            PyPlugin,
        )
            .into()
    }

    #[getter]
    pub fn power_preference(&self) -> Option<PyPowerPreference> {
        self.power_preference
    }

    #[getter]
    pub fn synchronous_pipeline_compilation(&self) -> bool {
        self.synchronous_pipeline_compilation
    }

    pub fn __repr__(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref pp) = self.power_preference {
            parts.push(format!("power_preference={}", object_repr(*pp)));
        }
        if self.synchronous_pipeline_compilation {
            parts.push(format!(
                "synchronous_pipeline_compilation={}",
                true.repr_literal()
            ));
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
        app.add_plugins(bevy::render::RenderPlugin {
            render_creation: bevy::render::settings::RenderCreation::Automatic(Box::new(
                wgpu_settings,
            )),
            synchronous_pipeline_compilation: config.synchronous_pipeline_compilation,
            ..Default::default()
        });
        Ok(())
    }
}
