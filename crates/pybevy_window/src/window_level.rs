use bevy::window::WindowLevel;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(WindowLevel)]
#[pyclass(name = "WindowLevel", eq)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PyWindowLevel {
    AlwaysOnBottom,
    #[default]
    Normal,
    AlwaysOnTop,
}
