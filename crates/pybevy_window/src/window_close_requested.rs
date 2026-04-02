use bevy::window::WindowCloseRequested;
use pybevy_core::{PyEntity, PyMessage};
use pybevy_macros::message_storage;
use pyo3::prelude::*;

#[message_storage(WindowCloseRequested)]
#[pyclass(name = "WindowCloseRequested", extends = PyMessage, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWindowCloseRequested {
    pub window: PyEntity,
}

impl From<&WindowCloseRequested> for PyWindowCloseRequested {
    fn from(event: &WindowCloseRequested) -> Self {
        PyWindowCloseRequested {
            window: event.window.into(),
        }
    }
}

#[pymethods]
impl PyWindowCloseRequested {
    #[new]
    fn new(window: PyEntity) -> (Self, PyMessage) {
        (PyWindowCloseRequested { window }, PyMessage)
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    fn __repr__(&self) -> String {
        "WindowCloseRequested()".to_string()
    }
}
