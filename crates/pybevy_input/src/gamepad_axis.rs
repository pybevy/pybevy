use bevy::input::gamepad::GamepadAxis;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(GamepadAxis, empty_tuple, from_only)]
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

    pub fn __repr__(&self) -> String {
        match self {
            PyGamepadAxis::LeftStickX() => "GamepadAxis.LeftStickX".to_string(),
            PyGamepadAxis::LeftStickY() => "GamepadAxis.LeftStickY".to_string(),
            PyGamepadAxis::LeftZ() => "GamepadAxis.LeftZ".to_string(),
            PyGamepadAxis::RightStickX() => "GamepadAxis.RightStickX".to_string(),
            PyGamepadAxis::RightStickY() => "GamepadAxis.RightStickY".to_string(),
            PyGamepadAxis::RightZ() => "GamepadAxis.RightZ".to_string(),
            PyGamepadAxis::Other(v) => format!("GamepadAxis.Other({})", v),
        }
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
