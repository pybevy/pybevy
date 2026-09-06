use bevy::window::WindowLevel;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(WindowLevel)]
#[pyclass(
    name = "WindowLevel",
    module = "pybevy.window",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PyWindowLevel {
    AlwaysOnBottom,
    #[default]
    Normal,
    AlwaysOnTop,
}
