use bevy::text::GenericFontFamily;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(GenericFontFamily)]
#[pyclass(
    name = "GenericFontFamily",
    module = "pybevy.text",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyGenericFontFamily {
    Serif,
    SansSerif,
    Cursive,
    Fantasy,
    Monospace,
    SystemUi,
    UiSerif,
    UiSansSerif,
    UiMonospace,
    UiRounded,
    Emoji,
    Math,
    FangSong,
}
