use bevy::window::WindowCloseRequested;
use pybevy_core::{PyEntity, PyMessage};
use pybevy_macros::pymessage;
use pyo3::prelude::*;

#[pymessage(WindowCloseRequested)]
#[pyclass(name = "WindowCloseRequested", module = "pybevy.window", extends = PyMessage, eq, skip_from_py_object)]
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
    fn new(window: PyEntity) -> PyClassInitializer<Self> {
        (PyWindowCloseRequested { window }, PyMessage).into()
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    fn __repr__(&self) -> String {
        "WindowCloseRequested()".to_string()
    }
}
