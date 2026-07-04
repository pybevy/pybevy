use bevy::text::FontAtlasSet;
use pybevy_core::ResourceStorage;
use pybevy_macros::pyresource;
use pyo3::prelude::*;

use crate::font_atlas::{PyFontAtlas, PyFontAtlasKey};

#[pyresource(FontAtlasSet, no_clone, bridge, no_mut, no_insert, no_default)]
#[pyclass(name = "FontAtlasSet", extends = pybevy_core::PyResource)]
pub struct PyFontAtlasSet {
    pub(crate) storage: ResourceStorage<FontAtlasSet>,
}

#[pymethods]
impl PyFontAtlasSet {
    pub fn items(&self) -> PyResult<Vec<(PyFontAtlasKey, Vec<PyFontAtlas>)>> {
        Ok(self
            .as_ref()?
            .iter()
            .map(|(key, atlases)| {
                (
                    key.into(),
                    atlases.iter().map(PyFontAtlas::from_bevy).collect(),
                )
            })
            .collect())
    }

    fn __iter__(&self) -> PyResult<PyFontAtlasSetKeyIter> {
        Ok(PyFontAtlasSetKeyIter {
            keys: self.as_ref()?.iter().map(|(key, _)| key.into()).collect(),
            index: 0,
        })
    }

    fn __len__(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.len())
    }

    pub fn __repr__(&self) -> String {
        "FontAtlasSet(...)".to_string()
    }
}

#[pyclass(name = "FontAtlasSetKeyIter")]
pub struct PyFontAtlasSetKeyIter {
    keys: Vec<PyFontAtlasKey>,
    index: usize,
}

#[pymethods]
impl PyFontAtlasSetKeyIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<PyFontAtlasKey> {
        if slf.index < slf.keys.len() {
            let key = slf.keys[slf.index].clone();
            slf.index += 1;
            Some(key)
        } else {
            None
        }
    }
}
