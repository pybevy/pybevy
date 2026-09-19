use bevy::input::gamepad::{GamepadAxis, GamepadButton, GamepadInput};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::{gamepad_axis::PyGamepadAxis, gamepad_button::PyGamepadButton};

#[pyenum(GamepadInput, no_repr)]
#[pyclass(
    name = "GamepadInput",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyGamepadInput {
    #[py_bevy(tuple)]
    Axis {
        #[py_type(PyGamepadAxis)]
        axis: GamepadAxis,
    },
    #[py_bevy(tuple)]
    Button {
        #[py_type(PyGamepadButton)]
        button: GamepadButton,
    },
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
