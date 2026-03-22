use bevy::text::FontSmoothing;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(FontSmoothing)]
#[pyclass(name = "FontSmoothing", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyFontSmoothing {
    None,
    AntiAliased,
}
