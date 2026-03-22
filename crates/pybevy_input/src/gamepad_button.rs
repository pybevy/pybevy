use bevy::input::gamepad::GamepadButton;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(GamepadButton, empty_tuple, from_only)]
#[pyclass(name = "GamepadButton", eq, frozen)]
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
    Other(u8),
}

#[pymethods]
impl PyGamepadButton {
    #[staticmethod]
    fn all() -> Vec<Self> {
        GamepadButton::all().into_iter().map(Into::into).collect()
    }

    pub fn __repr__(&self) -> String {
        match self {
            PyGamepadButton::South() => "GamepadButton.South".to_string(),
            PyGamepadButton::East() => "GamepadButton.East".to_string(),
            PyGamepadButton::North() => "GamepadButton.North".to_string(),
            PyGamepadButton::West() => "GamepadButton.West".to_string(),
            PyGamepadButton::C() => "GamepadButton.C".to_string(),
            PyGamepadButton::Z() => "GamepadButton.Z".to_string(),
            PyGamepadButton::LeftTrigger() => "GamepadButton.LeftTrigger".to_string(),
            PyGamepadButton::LeftTrigger2() => "GamepadButton.LeftTrigger2".to_string(),
            PyGamepadButton::RightTrigger() => "GamepadButton.RightTrigger".to_string(),
            PyGamepadButton::RightTrigger2() => "GamepadButton.RightTrigger2".to_string(),
            PyGamepadButton::Select() => "GamepadButton.Select".to_string(),
            PyGamepadButton::Start() => "GamepadButton.Start".to_string(),
            PyGamepadButton::Mode() => "GamepadButton.Mode".to_string(),
            PyGamepadButton::LeftThumb() => "GamepadButton.LeftThumb".to_string(),
            PyGamepadButton::RightThumb() => "GamepadButton.RightThumb".to_string(),
            PyGamepadButton::DPadUp() => "GamepadButton.DPadUp".to_string(),
            PyGamepadButton::DPadDown() => "GamepadButton.DPadDown".to_string(),
            PyGamepadButton::DPadLeft() => "GamepadButton.DPadLeft".to_string(),
            PyGamepadButton::DPadRight() => "GamepadButton.DPadRight".to_string(),
            PyGamepadButton::Other(v) => format!("GamepadButton.Other({})", v),
        }
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
            PyGamepadButton::Other(v) => format!("Other({})", v),
        }
    }
}
