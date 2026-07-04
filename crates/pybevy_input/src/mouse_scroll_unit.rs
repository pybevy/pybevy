use bevy::input::mouse::MouseScrollUnit;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(MouseScrollUnit)]
#[pyclass(name = "MouseScrollUnit", eq, frozen, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyMouseScrollUnit {
    Line,
    Pixel,
}
