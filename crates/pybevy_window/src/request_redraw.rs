use bevy::window::RequestRedraw;
use pybevy_core::PyMessage;
use pybevy_macros::message_storage;
use pyo3::prelude::*;

#[message_storage(RequestRedraw)]
#[pyclass(name = "RequestRedraw", extends = PyMessage, eq)]
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
    fn new() -> (Self, PyMessage) {
        (PyRequestRedraw, PyMessage)
    }

    fn __repr__(&self) -> String {
        "RequestRedraw()".to_string()
    }
}
