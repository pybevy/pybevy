use bevy::window::WindowFocused;
use pybevy_core::{PyEntity, PyMessage};
use pybevy_macros::pymessage;
use pyo3::prelude::*;

#[pymessage(WindowFocused)]
#[pyclass(name = "WindowFocused", module = "pybevy.window", extends = PyMessage, eq, skip_from_py_object)]
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
    fn new(focused: bool, window: PyEntity) -> PyClassInitializer<Self> {
        (PyWindowFocused { focused, window }, PyMessage).into()
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
