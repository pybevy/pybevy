use bevy::render::view::DebandDither;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;

#[pywrap(DebandDither, bridge, copy)]
#[pyclass(name = "DebandDither", module = "pybevy.render", extends = PyComponent, frozen, eq, hash, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyDebandDither(pub(crate) DebandDither);

#[pymethods]
impl PyDebandDither {
    #[new]
    pub fn new(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(DebandDither::default()))
    }

    #[classattr]
    #[pyo3(name = "Disabled")]
    pub fn disabled(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(DebandDither::Disabled))
    }

    #[classattr]
    #[pyo3(name = "Enabled")]
    pub fn enabled(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(DebandDither::Enabled))
    }

    pub fn __repr__(&self) -> &'static str {
        match self.0 {
            DebandDither::Disabled => "DebandDither.Disabled",
            DebandDither::Enabled => "DebandDither.Enabled",
        }
    }
}
