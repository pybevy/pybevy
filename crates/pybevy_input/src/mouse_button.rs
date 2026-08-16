use bevy::input::mouse::MouseButton;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(MouseButton, empty_tuple)]
#[pyclass(
    name = "MouseButton",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object,
    hash
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyMouseButton {
    Left(),
    Right(),
    Middle(),
    Back(),
    Forward(),
    #[py_bevy(tuple)]
    Other {
        value: u16,
    },
}
