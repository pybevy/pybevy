use bevy::input::mouse::MouseButton;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(MouseButton, empty_tuple)]
#[pyclass(name = "MouseButton", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyMouseButton {
    Left(),
    Right(),
    Middle(),
    Back(),
    Forward(),
    Other(u16),
}
