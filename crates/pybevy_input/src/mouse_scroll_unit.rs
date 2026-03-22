use bevy::input::mouse::MouseScrollUnit;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(MouseScrollUnit)]
#[pyclass(name = "MouseScrollUnit", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyMouseScrollUnit {
    Line,
    Pixel,
}
