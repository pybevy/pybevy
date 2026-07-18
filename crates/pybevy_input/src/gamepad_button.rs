use bevy::input::gamepad::GamepadButton;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(GamepadButton, empty_tuple)]
#[pyclass(
    name = "GamepadButton",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyGamepadButton {
    South(),
    East(),
    North(),
    West(),
    C(),
    Z(),
    LeftTrigger(),
    LeftTrigger2(),
    RightTrigger(),
    RightTrigger2(),
    Select(),
    Start(),
    Mode(),
    LeftThumb(),
    RightThumb(),
    DPadUp(),
    DPadDown(),
    DPadLeft(),
    DPadRight(),
    Other { value: u8 },
}

#[pymethods]
impl PyGamepadButton {
    #[staticmethod]
    fn all() -> Vec<Self> {
        GamepadButton::all().into_iter().map(Into::into).collect()
    }

    pub fn __str__(&self) -> String {
        match self {
            PyGamepadButton::South() => "South".to_string(),
            PyGamepadButton::East() => "East".to_string(),
            PyGamepadButton::North() => "North".to_string(),
            PyGamepadButton::West() => "West".to_string(),
            PyGamepadButton::C() => "C".to_string(),
            PyGamepadButton::Z() => "Z".to_string(),
            PyGamepadButton::LeftTrigger() => "LeftTrigger".to_string(),
            PyGamepadButton::LeftTrigger2() => "LeftTrigger2".to_string(),
            PyGamepadButton::RightTrigger() => "RightTrigger".to_string(),
            PyGamepadButton::RightTrigger2() => "RightTrigger2".to_string(),
            PyGamepadButton::Select() => "Select".to_string(),
            PyGamepadButton::Start() => "Start".to_string(),
            PyGamepadButton::Mode() => "Mode".to_string(),
            PyGamepadButton::LeftThumb() => "LeftThumb".to_string(),
            PyGamepadButton::RightThumb() => "RightThumb".to_string(),
            PyGamepadButton::DPadUp() => "DPadUp".to_string(),
            PyGamepadButton::DPadDown() => "DPadDown".to_string(),
            PyGamepadButton::DPadLeft() => "DPadLeft".to_string(),
            PyGamepadButton::DPadRight() => "DPadRight".to_string(),
            PyGamepadButton::Other { value } => format!("Other({})", value),
        }
    }
}
