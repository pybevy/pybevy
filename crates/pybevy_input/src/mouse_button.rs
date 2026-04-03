use bevy::input::mouse::MouseButton;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(MouseButton, empty_tuple)]
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
