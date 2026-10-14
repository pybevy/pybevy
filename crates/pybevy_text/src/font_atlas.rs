use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use bevy::{
    image::TextureAtlasLayout,
    text::{FontAtlas, FontAtlasKey},
};
use pybevy_core::{AssetStorage, FromBorrowedStorage, PyHandle, ValueStorage, repr::object_repr};
use pybevy_image::texture_atlas_layout::PyTextureAtlasLayout;
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

use crate::{font_hinting::PyFontHinting, font_smoothing::PyFontSmoothing};

#[pyclass(
    name = "FontAtlas",
    module = "pybevy.text",
    frozen,
    skip_from_py_object
)]
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
            PyTextureAtlasLayout::from_borrowed(AssetStorage::read_only_snapshot(
                self.texture_atlas.clone(),
            )),
        )
    }

    #[getter]
    pub fn texture(&self) -> PyHandle {
        self.texture.clone()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "FontAtlas(texture_atlas=TextureAtlasLayout(size={:?}, texture_count={}), texture={})",
            self.texture_atlas.size,
            self.texture_atlas.textures.len(),
            object_repr(self.texture.clone())
        )
    }
}

#[pyvalue]
#[pyclass(
    name = "FontAtlasKey",
    module = "pybevy.text",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyFontAtlasKey {
    pub(crate) storage: ValueStorage<FontAtlasKey>,
}

impl PartialEq for PyFontAtlasKey {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.to_bevy(), other.to_bevy()), (Ok(a), Ok(b)) if a == b)
    }
}

impl From<&FontAtlasKey> for PyFontAtlasKey {
    fn from(key: &FontAtlasKey) -> Self {
        Self::from_owned(*key)
    }
}

#[pymethods]
impl PyFontAtlasKey {
    #[getter]
    pub fn id(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.id)
    }

    #[getter]
    pub fn index(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.index)
    }

    #[getter]
    pub fn font_size_bits(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.font_size_bits)
    }

    #[getter]
    pub fn variations_hash(&self) -> PyResult<u64> {
        Ok(self.as_ref()?.variations_hash)
    }

    #[getter]
    pub fn hinting(&self) -> PyResult<PyFontHinting> {
        Ok(self.as_ref()?.hinting.into())
    }

    #[getter]
    pub fn font_smoothing(&self) -> PyResult<PyFontSmoothing> {
        Ok(self.as_ref()?.font_smoothing.into())
    }

    pub fn __hash__(&self) -> PyResult<u64> {
        let mut hasher = DefaultHasher::new();
        self.to_bevy()?.hash(&mut hasher);
        Ok(hasher.finish())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let key = self.to_bevy()?;
        Ok(format!(
            "FontAtlasKey(id={}, index={}, font_size_bits={}, variations_hash={}, hinting={}, font_smoothing={})",
            key.id,
            key.index,
            key.font_size_bits,
            key.variations_hash,
            object_repr(PyFontHinting::from(key.hinting)),
            object_repr(PyFontSmoothing::from(key.font_smoothing))
        ))
    }
}
