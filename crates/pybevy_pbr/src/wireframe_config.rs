use bevy::{color::Color, pbr::wireframe::WireframeConfig};
use pybevy_color::PyColor;
use pybevy_core::{PyResource, ResourceStorage};
use pybevy_macros::resource_storage;
use pyo3::prelude::*;

#[resource_storage(WireframeConfig)]
#[pyclass(name = "WireframeConfig", extends = PyResource)]
#[derive(Debug)]
pub struct PyWireframeConfig {
    pub storage: ResourceStorage<WireframeConfig>,
}

#[pymethods]
impl PyWireframeConfig {
    #[new]
    #[pyo3(signature = (global_ = false, default_color = PyColor(Color::WHITE)))]
    pub fn new(global_: bool, default_color: PyColor) -> (Self, PyResource) {
        Self::from_owned(WireframeConfig {
            global: global_,
            default_color: default_color.0,
        })
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
        PyColor::from_color(self.as_ref()?.default_color, py)
    }

    #[setter]
    pub fn set_default_color(&mut self, value: PyColor) -> PyResult<()> {
        self.as_mut()?.default_color = value.0;
        Ok(())
    }
}
