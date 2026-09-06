use bevy::text::FontSmoothing;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(FontSmoothing)]
#[pyclass(
    name = "FontSmoothing",
    module = "pybevy.text",
    eq,
    from_py_object,
    frozen,
    hash
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyFontSmoothing {
    #[pyo3(name = "None_")]
    None,
    AntiAliased,
}
