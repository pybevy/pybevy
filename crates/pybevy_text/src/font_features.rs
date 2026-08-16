use bevy::{
    reflect::{PartialReflect, ReflectRef},
    text::{FontFeatureTag, FontFeatures},
};
use pyo3::{exceptions::PyRuntimeError, prelude::*};

use crate::font_feature_tag::{PyFontFeatureTag, tag_characters};

#[pyclass(name = "FontFeatures", frozen, from_py_object)]
#[derive(Debug, Clone, Default)]
pub struct PyFontFeatures {
    features: Vec<(FontFeatureTag, u32)>,
}

#[pyclass(name = "FontFeaturesBuilder", from_py_object)]
#[derive(Debug, Clone, Default)]
pub struct PyFontFeaturesBuilder {
    features: Vec<(FontFeatureTag, u32)>,
}

impl From<PyFontFeatures> for FontFeatures {
    fn from(py_features: PyFontFeatures) -> Self {
        let mut builder = FontFeatures::builder();
        for (tag, value) in py_features.features {
            builder = builder.set(tag, value);
        }
        builder.build()
    }
}

/// `FontFeatures` keeps its entries private with no accessor, but derives
/// `Reflect` and marks nothing ignored, so the field is reachable that way.
impl TryFrom<&FontFeatures> for PyFontFeatures {
    type Error = PyErr;

    fn try_from(features: &FontFeatures) -> PyResult<Self> {
        let ReflectRef::Struct(reflected) = features.reflect_ref() else {
            return Err(PyRuntimeError::new_err(
                "FontFeatures no longer reflects as a struct",
            ));
        };
        let entries = reflected
            .field("features")
            .and_then(|field| field.try_downcast_ref::<Vec<(FontFeatureTag, u32)>>())
            .ok_or_else(|| {
                PyRuntimeError::new_err("FontFeatures no longer stores a list of tagged entries")
            })?;
        Ok(PyFontFeatures {
            features: entries.clone(),
        })
    }
}

impl PyFontFeaturesBuilder {
    fn extended(&self, tag: FontFeatureTag, value: u32) -> Self {
        let mut features = self.features.clone();
        features.push((tag, value));
        Self { features }
    }
}

#[pymethods]
impl PyFontFeatures {
    #[new]
    pub fn new() -> Self {
        Self::default()
    }

    #[staticmethod]
    pub fn builder() -> PyFontFeaturesBuilder {
        PyFontFeaturesBuilder::default()
    }

    fn __repr__(&self) -> String {
        if self.features.is_empty() {
            return "FontFeatures()".to_string();
        }
        let tags: Vec<String> = self
            .features
            .iter()
            .map(|(tag, value)| {
                let tag_str = tag_characters(tag);
                if *value == 1 {
                    format!("\"{}\"", tag_str)
                } else {
                    format!("\"{}\"={}", tag_str, value)
                }
            })
            .collect();
        format!("FontFeatures({})", tags.join(", "))
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.features == other.features
    }
}

#[pymethods]
impl PyFontFeaturesBuilder {
    #[new]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enable(&self, feature_tag: PyRef<'_, PyFontFeatureTag>) -> Self {
        self.extended(feature_tag.0, 1)
    }

    #[pyo3(signature = (feature_tag, value))]
    pub fn set(&self, feature_tag: PyRef<'_, PyFontFeatureTag>, value: u32) -> Self {
        self.extended(feature_tag.0, value)
    }

    pub fn build(&self) -> PyFontFeatures {
        PyFontFeatures {
            features: self.features.clone(),
        }
    }
}
