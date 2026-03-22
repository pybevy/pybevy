use bevy::input::gamepad::GamepadInput;
use pyo3::prelude::*;

use crate::{gamepad_axis::PyGamepadAxis, gamepad_button::PyGamepadButton};

#[pyclass(name = "GamepadInput", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyGamepadInput {
    Axis(PyGamepadAxis),
    Button(PyGamepadButton),
}

impl From<GamepadInput> for PyGamepadInput {
    fn from(input: GamepadInput) -> Self {
        match input {
            GamepadInput::Axis(axis) => PyGamepadInput::Axis(axis.into()),
            GamepadInput::Button(button) => PyGamepadInput::Button(button.into()),
        }
    }
}

impl From<PyGamepadInput> for GamepadInput {
    fn from(input: PyGamepadInput) -> Self {
        match input {
            PyGamepadInput::Axis(axis) => GamepadInput::Axis(axis.into()),
            PyGamepadInput::Button(button) => GamepadInput::Button(button.into()),
        }
    }
}

#[pymethods]
impl PyGamepadInput {
    #[staticmethod]
    pub fn from_axis(axis: PyGamepadAxis) -> Self {
        PyGamepadInput::Axis(axis)
    }

    #[staticmethod]
    pub fn from_button(button: PyGamepadButton) -> Self {
        PyGamepadInput::Button(button)
    }

    pub fn axis(&self) -> Option<PyGamepadAxis> {
        match self {
            PyGamepadInput::Axis(a) => Some(*a),
            PyGamepadInput::Button(_) => None,
        }
    }

    pub fn button(&self) -> Option<PyGamepadButton> {
        match self {
            PyGamepadInput::Axis(_) => None,
            PyGamepadInput::Button(b) => Some(*b),
        }
    }

    pub fn is_axis(&self) -> bool {
        matches!(self, PyGamepadInput::Axis(_))
    }

    pub fn is_button(&self) -> bool {
        matches!(self, PyGamepadInput::Button(_))
    }

    fn __repr__(&self) -> String {
        match self {
            PyGamepadInput::Axis(a) => format!("GamepadInput.Axis({:?})", a),
            PyGamepadInput::Button(b) => format!("GamepadInput.Button({:?})", b),
        }
    }
}
