use bevy::text::{FontAtlas, FontAtlasKey};
use pybevy_core::PyHandle;
use pyo3::prelude::*;

use crate::PyFontSmoothing;

#[pyclass(name = "FontAtlas", frozen)]
#[derive(Clone)]
pub struct PyFontAtlas {
    texture: PyHandle,
    texture_atlas: PyHandle,
}

impl PyFontAtlas {
    pub fn from_bevy(atlas: &FontAtlas) -> Self {
        PyFontAtlas {
            texture: (&atlas.texture).into(),
            texture_atlas: (&atlas.texture_atlas).into(),
        }
    }
}

#[pymethods]
impl PyFontAtlas {
    #[getter]
    pub fn texture(&self) -> PyHandle {
        self.texture.clone()
    }

    #[getter]
    pub fn texture_atlas(&self) -> PyHandle {
        self.texture_atlas.clone()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "FontAtlas(texture={:?}, texture_atlas={:?})",
            self.texture, self.texture_atlas
        )
    }
}

#[pyclass(name = "FontAtlasKey", frozen, eq)]
#[derive(Clone, PartialEq)]
pub struct PyFontAtlasKey {
    font: PyHandle,
    font_size_bits: u32,
    font_smoothing: PyFontSmoothing,
}

impl From<&FontAtlasKey> for PyFontAtlasKey {
    fn from(key: &FontAtlasKey) -> Self {
        PyFontAtlasKey {
            font: PyHandle::from(&key.0),
            font_size_bits: key.1,
            font_smoothing: key.2.into(),
        }
    }
}

#[pymethods]
impl PyFontAtlasKey {
    #[getter]
    pub fn font(&self) -> PyHandle {
        self.font.clone()
    }

    #[getter]
    pub fn font_size_bits(&self) -> u32 {
        self.font_size_bits
    }

    #[getter]
    pub fn font_smoothing(&self) -> PyFontSmoothing {
        self.font_smoothing
    }

    pub fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.font_size_bits.hash(&mut hasher);
        self.font_smoothing.hash(&mut hasher);
        hasher.finish()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "FontAtlasKey(font={:?}, font_size_bits={}, font_smoothing={:?})",
            self.font, self.font_size_bits, self.font_smoothing
        )
    }
}
