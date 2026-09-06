use bevy::window::ScreenEdge;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ScreenEdge)]
#[pyclass(
    name = "ScreenEdge",
    module = "pybevy.window",
    eq,
    from_py_object,
    frozen,
    hash
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyScreenEdge {
    #[pyo3(name = "None_")]
    None,
    Top,
    Left,
    Bottom,
    Right,
    All,
}
