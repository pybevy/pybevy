use bevy::input::mouse::MouseScrollUnit;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(MouseScrollUnit)]
#[pyclass(
    name = "MouseScrollUnit",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object,
    hash
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyMouseScrollUnit {
    Line,
    Pixel,
}
