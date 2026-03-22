use bevy::window::ScreenEdge;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(ScreenEdge)]
#[pyclass(name = "ScreenEdge", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyScreenEdge {
    None,
    Top,
    Left,
    Bottom,
    Right,
    All,
}
