use bevy::text::FontHinting;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(FontHinting)]
#[pyclass(name = "FontHinting", eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyFontHinting {
    Disabled,
    Enabled,
}
