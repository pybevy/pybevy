use bevy::sprite::Anchor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(Anchor, bridge)]
#[pyclass(name = "Anchor", extends = PyComponent, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyAnchor {
    pub(crate) storage: ComponentStorage<Anchor>,
}

#[pymethods]
impl PyAnchor {
    #[new]
    #[pyo3(signature = (value = PyVec2::ZERO))]
    pub fn new(value: PyVec2) -> (Self, PyComponent) {
        Self::from_owned(Anchor(value.into()))
    }

    #[staticmethod]
    #[pyo3(name = "CENTER")]
    pub fn center(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Anchor::CENTER))
    }

    #[staticmethod]
    #[pyo3(name = "BOTTOM_LEFT")]
    pub fn bottom_left(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Anchor::BOTTOM_LEFT))
    }

    #[staticmethod]
    #[pyo3(name = "BOTTOM_CENTER")]
    pub fn bottom_center(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Anchor::BOTTOM_CENTER))
    }

    #[staticmethod]
    #[pyo3(name = "BOTTOM_RIGHT")]
    pub fn bottom_right(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Anchor::BOTTOM_RIGHT))
    }

    #[staticmethod]
    #[pyo3(name = "CENTER_LEFT")]
    pub fn center_left(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Anchor::CENTER_LEFT))
    }

    #[staticmethod]
    #[pyo3(name = "CENTER_RIGHT")]
    pub fn center_right(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Anchor::CENTER_RIGHT))
    }

    #[staticmethod]
    #[pyo3(name = "TOP_LEFT")]
    pub fn top_left(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Anchor::TOP_LEFT))
    }

    #[staticmethod]
    #[pyo3(name = "TOP_CENTER")]
    pub fn top_center(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Anchor::TOP_CENTER))
    }

    #[staticmethod]
    #[pyo3(name = "TOP_RIGHT")]
    pub fn top_right(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Anchor::TOP_RIGHT))
    }

    #[staticmethod]
    pub fn custom(value: PyVec2, py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(Anchor(value.into())))
    }

    #[getter]
    pub fn value(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|a| &a.0)?)
    }

    pub fn as_vec(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.as_vec().into())
    }
}
