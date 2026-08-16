use bevy::{
    reflect::{PartialReflect, ReflectRef},
    text::FontFeatureTag,
};
use pyo3::{exceptions::PyValueError, prelude::*};

#[pyclass(name = "FontFeatureTag", frozen, eq, skip_from_py_object)]
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

fn parse_font_feature_tag(tag: &str) -> PyResult<FontFeatureTag> {
    if !tag.is_ascii() {
        return Err(PyValueError::new_err(format!(
            "FontFeatureTag must be ASCII, got '{tag}'"
        )));
    }
    let bytes: &[u8; 4] = tag.as_bytes().try_into().map_err(|_| {
        PyValueError::new_err(format!(
            "FontFeatureTag must be exactly 4 ASCII characters, got {} ('{}')",
            tag.len(),
            tag
        ))
    })?;
    Ok(FontFeatureTag::new(bytes))
}

pub(crate) fn tag_characters(tag: &FontFeatureTag) -> String {
    let ReflectRef::TupleStruct(reflected) = tag.reflect_ref() else {
        return "????".to_string();
    };
    reflected
        .field(0)
        .and_then(|field| field.try_downcast_ref::<[u8; 4]>())
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .unwrap_or("????")
        .to_string()
}

#[pymethods]
impl PyFontFeatureTag {
    #[new]
    pub fn new(tag: &str) -> PyResult<Self> {
        Ok(PyFontFeatureTag(parse_font_feature_tag(tag)?))
    }

    // Common OpenType feature tags
    #[classattr]
    pub const STANDARD_LIGATURES: PyFontFeatureTag =
        PyFontFeatureTag(FontFeatureTag::STANDARD_LIGATURES);
    #[classattr]
    pub const CONTEXTUAL_LIGATURES: PyFontFeatureTag =
        PyFontFeatureTag(FontFeatureTag::CONTEXTUAL_LIGATURES);
    #[classattr]
    pub const DISCRETIONARY_LIGATURES: PyFontFeatureTag =
        PyFontFeatureTag(FontFeatureTag::DISCRETIONARY_LIGATURES);
    #[classattr]
    pub const CONTEXTUAL_ALTERNATES: PyFontFeatureTag =
        PyFontFeatureTag(FontFeatureTag::CONTEXTUAL_ALTERNATES);
    #[classattr]
    pub const STYLISTIC_ALTERNATES: PyFontFeatureTag =
        PyFontFeatureTag(FontFeatureTag::STYLISTIC_ALTERNATES);
    #[classattr]
    pub const SMALL_CAPS: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::SMALL_CAPS);
    #[classattr]
    pub const CAPS_TO_SMALL_CAPS: PyFontFeatureTag =
        PyFontFeatureTag(FontFeatureTag::CAPS_TO_SMALL_CAPS);
    #[classattr]
    pub const SWASH: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::SWASH);
    #[classattr]
    pub const TITLING_ALTERNATES: PyFontFeatureTag =
        PyFontFeatureTag(FontFeatureTag::TITLING_ALTERNATES);
    #[classattr]
    pub const FRACTIONS: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::FRACTIONS);
    #[classattr]
    pub const ORDINALS: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::ORDINALS);
    #[classattr]
    pub const SLASHED_ZERO: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::SLASHED_ZERO);
    #[classattr]
    pub const SUPERSCRIPT: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::SUPERSCRIPT);
    #[classattr]
    pub const SUBSCRIPT: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::SUBSCRIPT);
    #[classattr]
    pub const OLDSTYLE_FIGURES: PyFontFeatureTag =
        PyFontFeatureTag(FontFeatureTag::OLDSTYLE_FIGURES);
    #[classattr]
    pub const LINING_FIGURES: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::LINING_FIGURES);
    #[classattr]
    pub const PROPORTIONAL_FIGURES: PyFontFeatureTag =
        PyFontFeatureTag(FontFeatureTag::PROPORTIONAL_FIGURES);
    #[classattr]
    pub const TABULAR_FIGURES: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::TABULAR_FIGURES);
    #[classattr]
    pub const WEIGHT: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::WEIGHT);
    #[classattr]
    pub const WIDTH: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::WIDTH);
    #[classattr]
    pub const SLANT: PyFontFeatureTag = PyFontFeatureTag(FontFeatureTag::SLANT);

    pub fn __repr__(&self) -> String {
        format!("{:?}", self.0)
    }

    #[getter]
    pub fn value(&self) -> String {
        tag_characters(&self.0)
    }

    pub fn __str__(&self) -> String {
        tag_characters(&self.0)
    }
}
