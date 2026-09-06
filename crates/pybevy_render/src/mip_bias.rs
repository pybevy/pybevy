use bevy::render::camera::MipBias;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;

#[pywrap(MipBias, bridge)]
#[pyclass(name = "MipBias", module = "pybevy.render", extends = PyComponent, frozen, skip_from_py_object)]
#[derive(Clone)]
pub struct PyMipBias(pub(crate) MipBias);

#[pymethods]
impl PyMipBias {
    #[new]
    #[pyo3(signature = (value = -1.0))]
    pub fn new(value: f32) -> PyClassInitializer<Self> {
        (PyMipBias(MipBias(value)), PyComponent).into()
    }

    #[getter]
    pub fn value(&self) -> f32 {
        self.0.0
    }

    pub fn __repr__(&self) -> String {
        format!("MipBias({})", self.0.0)
    }
}
