use bevy::window::WindowTheme;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(WindowTheme)]
#[pyclass(
    name = "WindowTheme",
    module = "pybevy.window",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyWindowTheme {
    Light,
    Dark,
}
