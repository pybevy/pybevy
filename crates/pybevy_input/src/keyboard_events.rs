use bevy::input::keyboard::KeyboardFocusLost;
pub use pybevy_core::PyMessage;
use pybevy_macros::pymessage;
use pyo3::prelude::*;

#[pymessage(KeyboardFocusLost)]
#[pyclass(name = "KeyboardFocusLost", module = "pybevy.input", extends = PyMessage, eq, skip_from_py_object)]
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

impl From<&PyKeyboardFocusLost> for KeyboardFocusLost {
    fn from(_value: &PyKeyboardFocusLost) -> Self {
        KeyboardFocusLost
    }
}

#[pymethods]
impl PyKeyboardFocusLost {
    #[new]
    fn new() -> PyClassInitializer<Self> {
        (PyKeyboardFocusLost, PyMessage).into()
    }

    fn __repr__(&self) -> String {
        "KeyboardFocusLost()".to_string()
    }
}
