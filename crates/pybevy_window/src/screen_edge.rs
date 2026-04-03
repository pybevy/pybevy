use bevy::window::ScreenEdge;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ScreenEdge)]
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
