use bevy::input::keyboard::KeyboardFocusLost;
pub use pybevy_core::PyMessage;
use pybevy_macros::message_storage;
use pyo3::prelude::*;

#[message_storage(KeyboardFocusLost)]
#[pyclass(name = "KeyboardFocusLost", extends = PyMessage, eq)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyKeyboardFocusLost;

impl PyKeyboardFocusLost {
    pub fn from_bevy(_event: &KeyboardFocusLost) -> (Self, PyMessage) {
        (PyKeyboardFocusLost, PyMessage)
    }
}

impl From<&KeyboardFocusLost> for PyKeyboardFocusLost {
    fn from(_event: &KeyboardFocusLost) -> Self {
        PyKeyboardFocusLost
    }
}

#[pymethods]
impl PyKeyboardFocusLost {
    #[new]
    fn new() -> (Self, PyMessage) {
        (PyKeyboardFocusLost, PyMessage)
    }

    fn __repr__(&self) -> String {
        "KeyboardFocusLost()".to_string()
    }
}
