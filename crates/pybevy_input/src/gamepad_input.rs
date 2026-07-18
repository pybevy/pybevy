use bevy::input::gamepad::GamepadInput;
use pyo3::prelude::*;

use crate::{gamepad_axis::PyGamepadAxis, gamepad_button::PyGamepadButton};

#[pyclass(
    name = "GamepadInput",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyGamepadInput {
    Axis { axis: PyGamepadAxis },
    Button { button: PyGamepadButton },
}

impl From<GamepadInput> for PyGamepadInput {
    fn from(input: GamepadInput) -> Self {
        match input {
            GamepadInput::Axis(axis) => PyGamepadInput::Axis { axis: axis.into() },
            GamepadInput::Button(button) => PyGamepadInput::Button {
                button: button.into(),
            },
        }
    }
}

impl From<PyGamepadInput> for GamepadInput {
    fn from(input: PyGamepadInput) -> Self {
        match input {
            PyGamepadInput::Axis { axis } => GamepadInput::Axis(axis.into()),
            PyGamepadInput::Button { button } => GamepadInput::Button(button.into()),
        }
    }
}

#[pymethods]
impl PyGamepadInput {
    fn __repr__(&self) -> String {
        match self {
            PyGamepadInput::Axis { axis } => format!("GamepadInput.Axis({:?})", axis),
            PyGamepadInput::Button { button } => format!("GamepadInput.Button({:?})", button),
        }
    }
}
