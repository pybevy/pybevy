use bevy::camera::ClearColor;
use pybevy_color::color::PyColor;
use pybevy_core::{PyResource, ResourceStorage, ResourceStorageInner};
use pybevy_macros::resource_storage;
use pyo3::prelude::*;

#[resource_storage(ClearColor, bridge)]
#[pyclass(name = "ClearColor", extends = PyResource, eq)]
#[derive(Debug)]
pub struct PyClearColor {
    pub storage: ResourceStorage<ClearColor>,
}

impl PartialEq for PyClearColor {
    fn eq(&self, other: &Self) -> bool {
        match (&self.storage.inner, &other.storage.inner) {
            (
                ResourceStorageInner::Owned { data: a, .. },
                ResourceStorageInner::Owned { data: b, .. },
            ) => a.0 == b.0,
            _ => match (self.as_ref(), other.as_ref()) {
                (Ok(a), Ok(b)) => a.0 == b.0,
                _ => false,
            },
        }
    }
}

#[pymethods]
impl PyClearColor {
    #[new]
    #[pyo3(signature = (color = ClearColor::default().0.into()))]
    pub fn new(color: PyColor) -> (Self, PyResource) {
        Self::from_owned(ClearColor(color.into()))
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.0, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.0 = color.into();
        Ok(())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(cc) => format!("ClearColor({:?})", cc.0),
            Err(_) => "ClearColor(<invalid>)".to_string(),
        }
    }
}
