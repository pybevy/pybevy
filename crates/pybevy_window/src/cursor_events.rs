use bevy::window::{CursorEntered, CursorLeft};
use pybevy_core::PyEntity;
pub use pybevy_core::PyMessage;
use pybevy_macros::message_bridge;
use pyo3::prelude::*;

// ============================================================================
// CursorEntered
// ============================================================================

#[pyclass(name = "CursorEntered", extends = PyMessage, frozen, eq)]
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
    pub fn new(window: PyEntity) -> (Self, PyMessage) {
        (PyCursorEntered { window }, PyMessage)
    }

    pub fn __repr__(&self) -> String {
        format!("CursorEntered(window={:?})", self.window)
    }
}

// ============================================================================
// CursorLeft
// ============================================================================

#[pyclass(name = "CursorLeft", extends = PyMessage, frozen, eq)]
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
    pub fn new(window: PyEntity) -> (Self, PyMessage) {
        (PyCursorLeft { window }, PyMessage)
    }

    pub fn __repr__(&self) -> String {
        format!("CursorLeft(window={:?})", self.window)
    }
}

// Message bridges
message_bridge!(CursorEntered, PyCursorEntered);
message_bridge!(CursorLeft, PyCursorLeft);
