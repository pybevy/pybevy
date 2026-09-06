use bevy::ui::UiScale;
use pybevy_core::{PyResource, ResourceStorage, resource_initializer};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

fn clone_ui_scale(value: &UiScale) -> PyResult<UiScale> {
    Ok(UiScale(value.0))
}

#[pyresource(UiScale, no_clone, bridge, clone_with = clone_ui_scale)]
#[pyclass(name = "UiScale", module = "pybevy.ui", extends = PyResource)]
#[derive(Debug)]
pub struct PyUiScale {
    pub(crate) storage: ResourceStorage<UiScale>,
}

#[pymethods]
impl PyUiScale {
    #[new]
    #[pyo3(signature = (scale = 1.0))]
    pub fn new(scale: f32) -> PyClassInitializer<Self> {
        resource_initializer(Self {
            storage: ResourceStorage::owned(UiScale(scale)),
        })
    }

    #[getter]
    pub fn scale(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.0)
    }

    #[setter]
    pub fn set_scale(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.0 = value;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(s) => format!("UiScale({})", s.0),
            Err(_) => "UiScale(<invalid>)".to_string(),
        }
    }

    pub fn __float__(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.0 as f64)
    }
}
