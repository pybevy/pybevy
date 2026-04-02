use bevy::render::camera::MipBias;
use pybevy_core::PyComponent;
use pybevy_macros::newtype_storage;
use pyo3::prelude::*;

#[newtype_storage(MipBias, bridge)]
#[pyclass(name = "MipBias", extends = PyComponent, frozen)]
#[derive(Clone)]
pub struct PyMipBias(pub(crate) MipBias);

#[pymethods]
impl PyMipBias {
    #[new]
    #[pyo3(signature = (value = -1.0))]
    pub fn new(value: f32) -> (Self, PyComponent) {
        (PyMipBias(MipBias(value)), PyComponent)
    }

    #[getter]
    pub fn value(&self) -> f32 {
        self.0.0
    }

    pub fn __repr__(&self) -> String {
        format!("MipBias({})", self.0.0)
    }
}
