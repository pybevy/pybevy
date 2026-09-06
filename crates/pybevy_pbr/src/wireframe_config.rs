use bevy::{
    color::Color,
    pbr::wireframe::{WireframeConfig, WireframeTopology},
};
use pybevy_color::color::PyColor;
use pybevy_core::{PyResource, ResourceStorage};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

use crate::wireframe_topology::PyWireframeTopology;

#[pyresource(WireframeConfig, bridge)]
#[pyclass(name = "WireframeConfig", module = "pybevy.pbr", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyWireframeConfig {
    pub storage: ResourceStorage<WireframeConfig>,
}

#[pymethods]
impl PyWireframeConfig {
    #[new]
    #[pyo3(signature = (
        global_ = false,
        default_color = Color::WHITE.into(),
        default_line_width = 1.0,
        default_topology = None
    ))]
    pub fn new(
        global_: bool,
        default_color: PyColor,
        default_line_width: f32,
        default_topology: Option<PyRef<'_, PyWireframeTopology>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let default_color = default_color.try_into()?;
        Ok(Self::from_owned(WireframeConfig {
            global: global_,
            default_color,
            default_line_width,
            default_topology: default_topology
                .map(|topology| topology.0)
                .unwrap_or(WireframeTopology::Triangles),
        }))
    }

    #[getter]
    pub fn global_(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.global)
    }

    #[setter]
    pub fn set_global_(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.global = value;
        Ok(())
    }

    #[getter]
    pub fn default_color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_resource_field(&self.storage, |config| &config.default_color, py)
    }

    #[setter]
    pub fn set_default_color(&mut self, value: PyColor) -> PyResult<()> {
        let value = value.try_into()?;
        self.as_mut()?.default_color = value;
        Ok(())
    }

    #[getter]
    pub fn default_line_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.default_line_width)
    }

    #[setter]
    pub fn set_default_line_width(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.default_line_width = value;
        Ok(())
    }

    #[getter]
    pub fn default_topology(&self, py: Python<'_>) -> PyResult<Py<PyWireframeTopology>> {
        Py::new(
            py,
            PyWireframeTopology::from_owned(self.as_ref()?.default_topology),
        )
    }

    #[setter]
    pub fn set_default_topology(&mut self, value: PyRef<'_, PyWireframeTopology>) -> PyResult<()> {
        self.as_mut()?.default_topology = value.0;
        Ok(())
    }
}
