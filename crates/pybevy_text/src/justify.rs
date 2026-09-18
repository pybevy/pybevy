use bevy::text::Justify;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(Justify)]
#[pyclass(
    name = "Justify",
    module = "pybevy.text",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyJustify {
    Left,
    Center,
    Right,
    Justified,
    Start,
    End,
}
