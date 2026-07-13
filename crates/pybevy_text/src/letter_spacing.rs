use bevy::text::LetterSpacing;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pyo3::prelude::*;

#[pywrap(LetterSpacing, bridge, copy)]
#[pyclass(from_py_object, name = "LetterSpacing", extends = PyComponent, frozen, eq)]
#[derive(Clone, Copy, PartialEq)]
pub struct PyLetterSpacing(pub(crate) LetterSpacing);

#[pymethods]
impl PyLetterSpacing {
    #[new]
    pub fn new(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(LetterSpacing::default()))
    }

    #[staticmethod]
    #[pyo3(name = "Px")]
    pub fn px(py: Python, value: f32) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(LetterSpacing::Px(value)))
    }

    #[staticmethod]
    #[pyo3(name = "Rem")]
    pub fn rem(py: Python, value: f32) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(LetterSpacing::Rem(value)))
    }

    #[getter]
    pub fn value(&self) -> f32 {
        match self.0 {
            LetterSpacing::Px(v) | LetterSpacing::Rem(v) => v,
        }
    }

    pub fn __repr__(&self) -> String {
        match self.0 {
            LetterSpacing::Px(v) => format!("LetterSpacing.Px({v})"),
            LetterSpacing::Rem(v) => format!("LetterSpacing.Rem({v})"),
        }
    }
}
