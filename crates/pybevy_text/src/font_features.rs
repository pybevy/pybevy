use bevy::text::{FontFeatureTag, FontFeatures};
use pyo3::{exceptions::PyValueError, prelude::*};

/// Python wrapper for Bevy's FontFeatures.
///
/// Provides a builder-style API for specifying OpenType font features.
/// Features are accumulated internally and converted to Bevy's FontFeatures
/// via the builder pattern.
#[pyclass(name = "FontFeatures")]
#[derive(Debug, Clone, Default)]
pub struct PyFontFeatures {
    features: Vec<([u8; 4], u32)>,
}

impl From<PyFontFeatures> for FontFeatures {
    fn from(py_features: PyFontFeatures) -> Self {
        let mut builder = FontFeatures::builder();
        for (tag, value) in py_features.features {
            builder = builder.set(FontFeatureTag::new(&tag), value);
        }
        builder.build()
    }
}

impl From<FontFeatures> for PyFontFeatures {
    fn from(_features: FontFeatures) -> Self {
        // FontFeatures has private fields with no public accessor,
        // so we cannot reconstruct the feature list from an existing FontFeatures.
        // Return an empty PyFontFeatures as a fallback.
        // Users should construct FontFeatures from Python, not read them back.
        PyFontFeatures {
            features: Vec::new(),
        }
    }
}

/// Parse a 4-byte OpenType tag from a Python string.
fn parse_tag(tag: &str) -> PyResult<[u8; 4]> {
    let bytes = tag.as_bytes();
    if bytes.len() != 4 {
        return Err(PyValueError::new_err(format!(
            "OpenType feature tag must be exactly 4 ASCII characters, got '{}' (length {})",
            tag,
            bytes.len()
        )));
    }
    if !bytes.iter().all(|b| b.is_ascii()) {
        return Err(PyValueError::new_err(format!(
            "OpenType feature tag must be ASCII characters, got '{}'",
            tag
        )));
    }
    Ok([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[pymethods]
impl PyFontFeatures {
    #[new]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable an OpenType feature (sets its value to 1).
    ///
    /// Returns self for method chaining.
    pub fn enable(slf: Py<Self>, py: Python<'_>, tag: &str) -> PyResult<Py<Self>> {
        let tag_bytes = parse_tag(tag)?;
        slf.borrow_mut(py).features.push((tag_bytes, 1));
        Ok(slf)
    }

    /// Disable an OpenType feature (sets its value to 0).
    ///
    /// Returns self for method chaining.
    pub fn disable(slf: Py<Self>, py: Python<'_>, tag: &str) -> PyResult<Py<Self>> {
        let tag_bytes = parse_tag(tag)?;
        slf.borrow_mut(py).features.push((tag_bytes, 0));
        Ok(slf)
    }

    /// Set an OpenType feature to a specific value.
    ///
    /// For most features, `enable()` or `disable()` should be used instead.
    /// Some features like "wght" take numeric values.
    ///
    /// Returns self for method chaining.
    #[pyo3(signature = (tag, value))]
    pub fn set(slf: Py<Self>, py: Python<'_>, tag: &str, value: u32) -> PyResult<Py<Self>> {
        let tag_bytes = parse_tag(tag)?;
        slf.borrow_mut(py).features.push((tag_bytes, value));
        Ok(slf)
    }

    /// Standard ligatures ("liga").
    #[staticmethod]
    pub fn standard_ligatures() -> Self {
        Self {
            features: vec![(*b"liga", 1)],
        }
    }

    /// Small caps ("smcp").
    #[staticmethod]
    pub fn small_caps() -> Self {
        Self {
            features: vec![(*b"smcp", 1)],
        }
    }

    /// Oldstyle figures ("onum").
    #[staticmethod]
    pub fn oldstyle_figures() -> Self {
        Self {
            features: vec![(*b"onum", 1)],
        }
    }

    /// Tabular figures ("tnum").
    #[staticmethod]
    pub fn tabular_figures() -> Self {
        Self {
            features: vec![(*b"tnum", 1)],
        }
    }

    /// Slashed zero ("zero").
    #[staticmethod]
    pub fn slashed_zero() -> Self {
        Self {
            features: vec![(*b"zero", 1)],
        }
    }

    /// Fractions ("frac").
    #[staticmethod]
    pub fn fractions() -> Self {
        Self {
            features: vec![(*b"frac", 1)],
        }
    }

    fn __repr__(&self) -> String {
        if self.features.is_empty() {
            return "FontFeatures()".to_string();
        }
        let tags: Vec<String> = self
            .features
            .iter()
            .map(|(tag, value)| {
                let tag_str = std::str::from_utf8(tag).unwrap_or("????");
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
