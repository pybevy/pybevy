use bevy::window::WindowTheme;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(WindowTheme)]
#[pyclass(name = "WindowTheme", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyWindowTheme {
    Light,
    Dark,
}
