use bevy::window::WindowFocused;
use pybevy_core::{PyEntity, PyMessage};
use pybevy_macros::message_storage;
use pyo3::prelude::*;

#[message_storage(WindowFocused)]
#[pyclass(name = "WindowFocused", extends = PyMessage, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWindowFocused {
    pub focused: bool,
    pub window: PyEntity,
}

impl From<&WindowFocused> for PyWindowFocused {
    fn from(event: &WindowFocused) -> Self {
        PyWindowFocused {
            focused: event.focused,
            window: event.window.into(),
        }
    }
}

#[pymethods]
impl PyWindowFocused {
    #[new]
    fn new(focused: bool, window: PyEntity) -> (Self, PyMessage) {
        (PyWindowFocused { focused, window }, PyMessage)
    }

    #[getter]
    fn focused(&self) -> bool {
        self.focused
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    fn __repr__(&self) -> String {
        format!("WindowFocused(focused={})", self.focused)
    }
}
