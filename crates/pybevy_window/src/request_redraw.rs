use bevy::window::RequestRedraw;
use pybevy_core::PyMessage;
use pybevy_macros::pymessage;
use pyo3::prelude::*;

#[pymessage(RequestRedraw)]
#[pyclass(name = "RequestRedraw", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyRequestRedraw;

impl From<&RequestRedraw> for PyRequestRedraw {
    fn from(_event: &RequestRedraw) -> Self {
        PyRequestRedraw
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
