use bevy::text::FontStyle;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(FontStyle, empty_tuple)]
#[pyclass(name = "FontStyle", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyFontStyle {
    Normal(),
    Italic(),
    #[pyo3(constructor = (_0 = None))]
    Oblique(Option<f32>),
}
