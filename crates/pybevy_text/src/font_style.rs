use bevy::text::FontStyle;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(FontStyle, empty_tuple)]
#[pyclass(name = "FontStyle", module = "pybevy.text", eq, frozen, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyFontStyle {
    Normal(),
    Italic(),
    #[py_bevy(tuple)]
    #[pyo3(constructor = (value = None))]
    Oblique {
        value: Option<f32>,
    },
}
