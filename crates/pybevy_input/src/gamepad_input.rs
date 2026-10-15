use bevy::input::gamepad::{GamepadAxis, GamepadButton, GamepadInput};
use pybevy_macros::pyenum;
use pyo3::{exceptions::PyTypeError, prelude::*};

use crate::{gamepad_axis::PyGamepadAxis, gamepad_button::PyGamepadButton};

#[pyenum(GamepadInput, no_repr)]
#[pyclass(
    name = "GamepadInput",
    module = "pybevy.input",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
            PyGamepadInput::Axis { axis } => format!("GamepadInput.Axis({})", axis.__repr__()),
            PyGamepadInput::Button { button } => {
                format!("GamepadInput.Button({})", button.__repr__())
            }
        }
    }
}

pub fn extract_gamepad_input_from_any(obj: &Bound<'_, PyAny>) -> PyResult<GamepadInput> {
    if let Ok(input) = obj.extract::<PyGamepadInput>() {
        return Ok(input.into());
    }
    if let Ok(axis) = obj.extract::<PyGamepadAxis>() {
        return Ok(GamepadInput::from(GamepadAxis::from(axis)));
    }
    if let Ok(button) = obj.extract::<PyGamepadButton>() {
        return Ok(GamepadInput::from(GamepadButton::from(button)));
    }
    Err(PyTypeError::new_err(format!(
        "expected GamepadInput, GamepadAxis, or GamepadButton, not {}",
        obj.get_type().name()?
    )))
}
