use bevy::{
    image::TextureAtlasLayout,
    text::{FontAtlas, FontAtlasKey},
};
use pybevy_core::PyHandle;
use pybevy_image::texture_atlas_layout::PyTextureAtlasLayout;
use pyo3::prelude::*;

use crate::{font_hinting::PyFontHinting, font_smoothing::PyFontSmoothing};

#[pyclass(name = "FontAtlas", frozen)]
#[derive(Clone)]
pub struct PyFontAtlas {
    texture_atlas: TextureAtlasLayout,
    texture: PyHandle,
}

impl PyFontAtlas {
    pub fn from_bevy(atlas: &FontAtlas) -> Self {
        PyFontAtlas {
            texture_atlas: atlas.texture_atlas.clone(),
            texture: (&atlas.texture).into(),
        }
    }
}

#[pymethods]
impl PyFontAtlas {
    #[getter]
    pub fn texture_atlas(&self, py: Python<'_>) -> PyResult<Py<PyTextureAtlasLayout>> {
        Py::new(
            py,
            PyTextureAtlasLayout::from_owned(self.texture_atlas.clone()),
        )
    }

    #[getter]
    pub fn texture(&self) -> PyHandle {
        self.texture.clone()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "FontAtlas(texture_atlas=TextureAtlasLayout(size={:?}, texture_count={}), texture={:?})",
            self.texture_atlas.size,
            self.texture_atlas.textures.len(),
            self.texture
        )
    }
}

#[pyclass(name = "FontAtlasKey", frozen, eq)]
#[derive(Clone, PartialEq)]
pub struct PyFontAtlasKey {
    id: u32,
    index: u32,
    font_size_bits: u32,
    variations_hash: u64,
    hinting: PyFontHinting,
    font_smoothing: PyFontSmoothing,
}

impl From<&FontAtlasKey> for PyFontAtlasKey {
    fn from(key: &FontAtlasKey) -> Self {
        PyFontAtlasKey {
            id: key.id,
            index: key.index,
            font_size_bits: key.font_size_bits,
            variations_hash: key.variations_hash,
            hinting: key.hinting.into(),
            font_smoothing: key.font_smoothing.into(),
        }
    }
}

#[pymethods]
impl PyFontAtlasKey {
    #[getter]
    pub fn id(&self) -> u32 {
        self.id
    }

    #[getter]
    pub fn index(&self) -> u32 {
        self.index
    }

    #[getter]
    pub fn font_size_bits(&self) -> u32 {
        self.font_size_bits
    }

    #[getter]
    pub fn variations_hash(&self) -> u64 {
        self.variations_hash
    }

    #[getter]
    pub fn hinting(&self) -> PyFontHinting {
        self.hinting
    }

    #[getter]
    pub fn font_smoothing(&self) -> PyFontSmoothing {
        self.font_smoothing
    }

    pub fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        self.index.hash(&mut hasher);
        self.font_size_bits.hash(&mut hasher);
        self.variations_hash.hash(&mut hasher);
        self.hinting.hash(&mut hasher);
        self.font_smoothing.hash(&mut hasher);
        hasher.finish()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "FontAtlasKey(id={}, index={}, font_size_bits={}, variations_hash={}, hinting={:?}, font_smoothing={:?})",
            self.id,
            self.index,
            self.font_size_bits,
            self.variations_hash,
            self.hinting,
            self.font_smoothing
        )
    }
}
