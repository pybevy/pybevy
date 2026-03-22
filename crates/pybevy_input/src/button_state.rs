use bevy::input::ButtonState;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(ButtonState, empty_tuple, from_only)]
#[pyclass(name = "ButtonState", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyButtonState {
    Pressed(),
    Released(),
}

#[pymethods]
impl PyButtonState {
    pub fn is_pressed(&self) -> bool {
        matches!(self, PyButtonState::Pressed())
    }

    fn __repr__(&self) -> String {
        match self {
            PyButtonState::Pressed() => "ButtonState.Pressed".to_string(),
            PyButtonState::Released() => "ButtonState.Released".to_string(),
        }
    }

    fn __str__(&self) -> String {
        match self {
            PyButtonState::Pressed() => "Pressed".to_string(),
            PyButtonState::Released() => "Released".to_string(),
        }
    }
}
