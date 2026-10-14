use std::fmt;

use bevy::{
    app::App,
    render::{
        RenderPlugin,
        settings::{RenderCreation, WgpuSettings},
        texture::TexturePlugin,
    },
};
use pybevy_core::{Omittable, PluginBuild, PyPlugin, repr::ReprLiteral};
use pybevy_macros::pyplugin;
use pyo3::prelude::*;

use crate::render_creation::PyRenderCreation;

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
#[derive(Clone, Default)]
pub struct PyRenderPlugin {
    pub render_creation: Option<PyRenderCreation>,
    pub synchronous_pipeline_compilation: bool,
}

impl fmt::Debug for PyRenderPlugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PyRenderPlugin")
            .field(
                "synchronous_pipeline_compilation",
                &self.synchronous_pipeline_compilation,
            )
            .finish_non_exhaustive()
    }
}

#[pymethods]
impl PyRenderPlugin {
    #[new]
    #[pyo3(signature = (*, render_creation = Omittable::default(), synchronous_pipeline_compilation = false))]
    pub fn new(
        render_creation: Omittable<PyRef<'_, PyRenderCreation>>,
        synchronous_pipeline_compilation: bool,
    ) -> PyClassInitializer<Self> {
        (
            PyRenderPlugin {
                render_creation: render_creation
                    .into_option()
                    .map(|creation| (*creation).clone()),
                synchronous_pipeline_compilation,
            },
            PyPlugin,
        )
            .into()
    }

    #[getter]
    pub fn render_creation(&self, py: Python<'_>) -> PyResult<Py<PyRenderCreation>> {
        self.render_creation
            .clone()
            .unwrap_or_else(|| PyRenderCreation::automatic(WgpuSettings::default()))
            .into_object(py)
    }

    #[getter]
    pub fn synchronous_pipeline_compilation(&self) -> bool {
        self.synchronous_pipeline_compilation
    }

    pub fn __repr__(&self) -> String {
        let mut parts = Vec::new();
        if let Some(creation) = &self.render_creation {
            parts.push(format!("render_creation={}", creation.__repr__()));
        }
        if self.synchronous_pipeline_compilation {
            parts.push(format!(
                "synchronous_pipeline_compilation={}",
                true.repr_literal()
            ));
        }
        format!("RenderPlugin({})", parts.join(", "))
    }
}

impl PluginBuild for PyRenderPlugin {
    fn build(py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        let config: PyRef<'_, PyRenderPlugin> = py_plugin.extract()?;
        let render_creation = config
            .render_creation
            .as_ref()
            .map(RenderCreation::from)
            .unwrap_or_default();
        app.add_plugins(RenderPlugin {
            render_creation,
            synchronous_pipeline_compilation: config.synchronous_pipeline_compilation,
            ..Default::default()
        });
        Ok(())
    }
}
