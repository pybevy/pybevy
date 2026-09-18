use bevy::window::RequestRedraw;
use pybevy_core::PyMessage;
use pybevy_macros::pymessage;
use pyo3::prelude::*;

#[pymessage(RequestRedraw, writable)]
#[pyclass(name = "RequestRedraw", module = "pybevy.window", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyRequestRedraw;

impl From<&RequestRedraw> for PyRequestRedraw {
    fn from(_event: &RequestRedraw) -> Self {
        PyRequestRedraw
    }
}

impl TryFrom<&PyRequestRedraw> for RequestRedraw {
    type Error = PyErr;

    fn try_from(_value: &PyRequestRedraw) -> PyResult<Self> {
        Ok(RequestRedraw)
    }
}

#[pymethods]
impl PyRequestRedraw {
    #[new]
    fn new() -> PyClassInitializer<Self> {
        (PyRequestRedraw, PyMessage).into()
    }

    fn __repr__(&self) -> String {
        "RequestRedraw()".to_string()
    }
}
