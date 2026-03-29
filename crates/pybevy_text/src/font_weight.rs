use bevy::text::FontWeight;
use pyo3::prelude::*;

#[pyclass(name = "FontWeight", frozen, eq, hash)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyFontWeight(pub(crate) FontWeight);

impl From<FontWeight> for PyFontWeight {
    fn from(value: FontWeight) -> Self {
        PyFontWeight(value)
    }
}

impl From<PyFontWeight> for FontWeight {
    fn from(value: PyFontWeight) -> Self {
        value.0
    }
}

#[pymethods]
impl PyFontWeight {
    #[new]
    #[pyo3(signature = (value = 400))]
    pub fn new(value: u16) -> Self {
        PyFontWeight(FontWeight(value))
    }

    #[classattr]
    pub const THIN: PyFontWeight = PyFontWeight(FontWeight::THIN);

    #[classattr]
    pub const EXTRA_LIGHT: PyFontWeight = PyFontWeight(FontWeight::EXTRA_LIGHT);

    #[classattr]
    pub const LIGHT: PyFontWeight = PyFontWeight(FontWeight::LIGHT);

    #[classattr]
    pub const NORMAL: PyFontWeight = PyFontWeight(FontWeight::NORMAL);

    #[classattr]
    pub const MEDIUM: PyFontWeight = PyFontWeight(FontWeight::MEDIUM);

    #[classattr]
    pub const SEMIBOLD: PyFontWeight = PyFontWeight(FontWeight::SEMIBOLD);

    #[classattr]
    pub const BOLD: PyFontWeight = PyFontWeight(FontWeight::BOLD);

    #[classattr]
    pub const EXTRA_BOLD: PyFontWeight = PyFontWeight(FontWeight::EXTRA_BOLD);

    #[classattr]
    pub const BLACK: PyFontWeight = PyFontWeight(FontWeight::BLACK);

    #[classattr]
    pub const EXTRA_BLACK: PyFontWeight = PyFontWeight(FontWeight::EXTRA_BLACK);

    #[classattr]
    pub const DEFAULT: PyFontWeight = PyFontWeight(FontWeight::DEFAULT);

    #[getter]
    pub fn value(&self) -> u16 {
        self.0.0
    }

    pub fn clamp(&self) -> Self {
        PyFontWeight(self.0.clamp())
    }

    pub fn __repr__(&self) -> String {
        let name = match self.0.0 {
            100 => "THIN",
            200 => "EXTRA_LIGHT",
            300 => "LIGHT",
            400 => "NORMAL",
            500 => "MEDIUM",
            600 => "SEMIBOLD",
            700 => "BOLD",
            800 => "EXTRA_BOLD",
            900 => "BLACK",
            950 => "EXTRA_BLACK",
            _ => return format!("FontWeight({})", self.0.0),
        };
        format!("FontWeight.{name}")
    }
}
