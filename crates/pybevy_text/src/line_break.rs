use bevy::text::LineBreak;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(LineBreak)]
#[pyclass(name = "LineBreak", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyLineBreak {
    WordBoundary,
    AnyCharacter,
    WordOrCharacter,
    NoWrap,
}
