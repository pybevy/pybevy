use bevy::input::gamepad::GamepadAxis;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(GamepadAxis, empty_tuple)]
#[pyclass(name = "GamepadAxis", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyGamepadAxis {
    LeftStickX(),
    LeftStickY(),
    LeftZ(),
    RightStickX(),
    RightStickY(),
    RightZ(),
    Other(u8),
}

#[pymethods]
impl PyGamepadAxis {
    #[staticmethod]
    fn all() -> Vec<Self> {
        GamepadAxis::all().into_iter().map(Into::into).collect()
    }

    pub fn __str__(&self) -> String {
        match self {
            PyGamepadAxis::LeftStickX() => "LeftStickX".to_string(),
            PyGamepadAxis::LeftStickY() => "LeftStickY".to_string(),
            PyGamepadAxis::LeftZ() => "LeftZ".to_string(),
            PyGamepadAxis::RightStickX() => "RightStickX".to_string(),
            PyGamepadAxis::RightStickY() => "RightStickY".to_string(),
            PyGamepadAxis::RightZ() => "RightZ".to_string(),
            PyGamepadAxis::Other(v) => format!("Other({})", v),
        }
    }
}
