use bevy::input::keyboard::KeyboardInput;
use pybevy_core::PyMessage;
use pyo3::PyResult;

use crate::keyboard_input::PyKeyboardInput;
pub trait PyKeyboardInputExt {
    fn from_bevy(event: &KeyboardInput) -> PyResult<(PyKeyboardInput, PyMessage)>;
}

impl PyKeyboardInputExt for PyKeyboardInput {
    fn from_bevy(event: &KeyboardInput) -> PyResult<(PyKeyboardInput, PyMessage)> {
        Ok((PyKeyboardInput::from_bevy_event(event)?, PyMessage))
    }
}
