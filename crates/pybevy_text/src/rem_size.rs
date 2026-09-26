use bevy::text::RemSize;
use pybevy_core::{PyResource, ResourceStorage, resource_initializer};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

#[pyresource(RemSize, bridge, no_reflect)]
#[pyclass(name = "RemSize", module = "pybevy.text", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyRemSize {
    pub(crate) storage: ResourceStorage<RemSize>,
}

#[pymethods]
impl PyRemSize {
    #[new]
    #[pyo3(signature = (value = 20.0))]
    pub fn new(value: f32) -> PyClassInitializer<Self> {
        resource_initializer(Self {
            storage: ResourceStorage::owned(RemSize(value)),
        })
    }

    #[getter]
    pub fn value(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.0)
    }

    #[setter]
    pub fn set_value(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.0 = value;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(size) => format!("RemSize({})", size.0),
            Err(_) => "RemSize(<invalid>)".to_string(),
        }
    }
}
