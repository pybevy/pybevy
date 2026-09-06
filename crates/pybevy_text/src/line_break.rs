use bevy::text::LineBreak;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(LineBreak)]
#[pyclass(name = "LineBreak", module = "pybevy.text", eq, frozen, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyLineBreak {
    WordBoundary,
    AnyCharacter,
    WordOrCharacter,
    NoWrap,
}
