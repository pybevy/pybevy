use bevy::text::FontWidth;
use pyo3::prelude::*;

#[pyclass(name = "FontWidth", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyFontWidth(pub(crate) FontWidth);

impl From<FontWidth> for PyFontWidth {
    fn from(value: FontWidth) -> Self {
        PyFontWidth(value)
    }
}

impl From<PyFontWidth> for FontWidth {
    fn from(value: PyFontWidth) -> Self {
        value.0
    }
}

#[pymethods]
impl PyFontWidth {
    #[new]
    #[pyo3(signature = (value = 1.0))]
    pub fn new(value: f32) -> Self {
        PyFontWidth(FontWidth(value))
    }

    #[getter]
    pub fn value(&self) -> f32 {
        self.0.0
    }

    #[classattr]
    pub const ULTRA_CONDENSED: PyFontWidth = PyFontWidth(FontWidth::ULTRA_CONDENSED);

    #[classattr]
    pub const EXTRA_CONDENSED: PyFontWidth = PyFontWidth(FontWidth::EXTRA_CONDENSED);

    #[classattr]
    pub const CONDENSED: PyFontWidth = PyFontWidth(FontWidth::CONDENSED);

    #[classattr]
    pub const SEMI_CONDENSED: PyFontWidth = PyFontWidth(FontWidth::SEMI_CONDENSED);

    #[classattr]
    pub const NORMAL: PyFontWidth = PyFontWidth(FontWidth::NORMAL);

    #[classattr]
    pub const SEMI_EXPANDED: PyFontWidth = PyFontWidth(FontWidth::SEMI_EXPANDED);

    #[classattr]
    pub const EXPANDED: PyFontWidth = PyFontWidth(FontWidth::EXPANDED);

    #[classattr]
    pub const EXTRA_EXPANDED: PyFontWidth = PyFontWidth(FontWidth::EXTRA_EXPANDED);

    #[classattr]
    pub const ULTRA_EXPANDED: PyFontWidth = PyFontWidth(FontWidth::ULTRA_EXPANDED);

    pub fn __repr__(&self) -> String {
        let v = self.0;
        let name = if v == FontWidth::ULTRA_CONDENSED {
            "ULTRA_CONDENSED"
        } else if v == FontWidth::EXTRA_CONDENSED {
            "EXTRA_CONDENSED"
        } else if v == FontWidth::CONDENSED {
            "CONDENSED"
        } else if v == FontWidth::SEMI_CONDENSED {
            "SEMI_CONDENSED"
        } else if v == FontWidth::NORMAL {
            "NORMAL"
        } else if v == FontWidth::SEMI_EXPANDED {
            "SEMI_EXPANDED"
        } else if v == FontWidth::EXPANDED {
            "EXPANDED"
        } else if v == FontWidth::EXTRA_EXPANDED {
            "EXTRA_EXPANDED"
        } else if v == FontWidth::ULTRA_EXPANDED {
            "ULTRA_EXPANDED"
        } else {
            return format!("FontWidth({})", v.0);
        };
        format!("FontWidth.{name}")
    }
}
