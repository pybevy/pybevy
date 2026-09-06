use bevy::window::{CursorEntered, CursorLeft};
use pybevy_core::PyEntity;
pub use pybevy_core::PyMessage;
use pybevy_macros::pymessage;
use pyo3::prelude::*;

#[pymessage(CursorEntered)]
#[pyclass(name = "CursorEntered", module = "pybevy.window", extends = PyMessage, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyCursorEntered {
    #[pyo3(get)]
    pub window: PyEntity,
}

impl PyCursorEntered {
    pub fn from_bevy(event: &CursorEntered) -> (Self, PyMessage) {
        (Self::from(event), PyMessage)
    }
}

impl From<&CursorEntered> for PyCursorEntered {
    fn from(event: &CursorEntered) -> Self {
        PyCursorEntered {
            window: event.window.into(),
        }
    }
}

#[pymethods]
impl PyCursorEntered {
    #[new]
    pub fn new(window: PyEntity) -> PyClassInitializer<Self> {
        (PyCursorEntered { window }, PyMessage).into()
    }

    pub fn __repr__(&self) -> String {
        format!("CursorEntered(window={:?})", self.window)
    }
}

#[pymessage(CursorLeft)]
#[pyclass(name = "CursorLeft", module = "pybevy.window", extends = PyMessage, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyCursorLeft {
    #[pyo3(get)]
    pub window: PyEntity,
}

impl PyCursorLeft {
    pub fn from_bevy(event: &CursorLeft) -> (Self, PyMessage) {
        (Self::from(event), PyMessage)
    }
}

impl From<&CursorLeft> for PyCursorLeft {
    fn from(event: &CursorLeft) -> Self {
        PyCursorLeft {
            window: event.window.into(),
        }
    }
}

#[pymethods]
impl PyCursorLeft {
    #[new]
    pub fn new(window: PyEntity) -> PyClassInitializer<Self> {
        (PyCursorLeft { window }, PyMessage).into()
    }

    pub fn __repr__(&self) -> String {
        format!("CursorLeft(window={:?})", self.window)
    }
}
