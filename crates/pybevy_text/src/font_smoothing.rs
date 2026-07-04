use bevy::text::FontSmoothing;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(FontSmoothing)]
#[pyclass(name = "FontSmoothing", eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyFontSmoothing {
    None,
    AntiAliased,
}
