use bevy::text::Justify;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(Justify)]
#[pyclass(name = "Justify", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyJustify {
    Left,
    Center,
    Right,
    Justified,
}
