use bevy::text::FontFeatureTag;
use pyo3::{exceptions::PyValueError, prelude::*};

#[pyclass(name = "FontFeatureTag", frozen, eq, hash)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyFontFeatureTag(pub(crate) FontFeatureTag);

impl From<FontFeatureTag> for PyFontFeatureTag {
    fn from(tag: FontFeatureTag) -> Self {
        PyFontFeatureTag(tag)
    }
}

impl From<PyFontFeatureTag> for FontFeatureTag {
    fn from(tag: PyFontFeatureTag) -> Self {
        tag.0
    }
}

#[pymethods]
impl PyFontFeatureTag {
    #[new]
    pub fn new(tag: &str) -> PyResult<Self> {
        let bytes = tag.as_bytes();
        if bytes.len() != 4 {
            return Err(PyValueError::new_err(format!(
                "FontFeatureTag must be exactly 4 ASCII characters, got {} ('{}')",
                bytes.len(),
                tag
            )));
        }
        Ok(PyFontFeatureTag(FontFeatureTag::new(
            bytes.try_into().unwrap(),
        )))
    }

    // Common OpenType feature tags
    #[classattr]
    pub const STANDARD_LIGATURES: PyFontFeatureTag =
        PyFontFeatureTag(FontFeatureTag::STANDARD_LIGATURES);
    #[classattr]
    pub const SMALL_CAPS: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::SMALL_CAPS);
    #[classattr]
    pub const OLDSTYLE_FIGURES: PyFontFeatureTag =
        PyFontFeatureTag(FontFeatureTag::OLDSTYLE_FIGURES);
    #[classattr]
    pub const TABULAR_FIGURES: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::TABULAR_FIGURES);
    #[classattr]
    pub const FRACTIONS: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::FRACTIONS);
    #[classattr]
    pub const SLASHED_ZERO: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::SLASHED_ZERO);

    pub fn __repr__(&self) -> String {
        "FontFeatureTag(...)".to_string()
    }
}
