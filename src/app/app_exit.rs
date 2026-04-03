use std::num::NonZero;

use bevy::app::AppExit;
use pybevy_macros::pymessage;
use pyo3::prelude::*;

use crate::ecs::message::PyMessage;

#[pymessage(AppExit, writable)]
#[pyclass(name = "AppExit", extends = PyMessage, frozen, eq)]
#[derive(Debug, PartialEq)]
pub struct PyAppExit(pub(crate) AppExit);

impl From<AppExit> for PyAppExit {
    fn from(exit: AppExit) -> Self {
        Self(exit)
    }
}

impl From<&AppExit> for PyAppExit {
    fn from(exit: &AppExit) -> Self {
        Self(exit.clone())
    }
}

impl TryFrom<&PyAppExit> for AppExit {
    type Error = PyErr;

    fn try_from(py_exit: &PyAppExit) -> Result<Self, <Self as TryFrom<&PyAppExit>>::Error> {
        Ok(py_exit.0.clone())
    }
}

#[pymethods]
impl PyAppExit {
    #[new]
    pub fn new() -> (Self, PyMessage) {
        (Self(AppExit::Success), PyMessage)
    }

    #[classattr]
    #[allow(non_snake_case)]
    pub fn SUCCESS() -> PyResult<Py<Self>> {
        Python::attach(|py| Py::new(py, (Self(AppExit::Success), PyMessage)))
    }

    #[staticmethod]
    pub fn error(py: Python, code: NonZero<u8>) -> PyResult<Py<Self>> {
        Py::new(py, (Self(AppExit::Error(code)), PyMessage))
    }

    pub fn is_success(&self) -> bool {
        self.0.is_success()
    }

    pub fn is_error(&self) -> bool {
        self.0.is_error()
    }
}
