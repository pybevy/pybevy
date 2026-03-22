use bevy::{
    asset::AssetId,
    text::{Font, FontAtlasSet},
};
use pybevy_core::{PyResource, ResourceStorage, extract_handle_from_any};
use pyo3::prelude::*;

use crate::font_atlas::{PyFontAtlas, PyFontAtlasKey};

#[pyclass(name = "FontAtlasSet", extends = PyResource)]
pub struct PyFontAtlasSet {
    pub(crate) storage: ResourceStorage<FontAtlasSet>,
}

impl PyFontAtlasSet {
    pub fn from_borrowed(storage: ResourceStorage<FontAtlasSet>) -> (Self, PyResource) {
        (Self { storage }, PyResource)
    }

    #[inline(always)]
    pub(crate) fn as_ref(&self) -> PyResult<&FontAtlasSet> {
        Ok(self.storage.as_ref()?)
    }
}

#[pymethods]
impl PyFontAtlasSet {
    pub fn get_by_font(
        &self,
        id: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<(PyFontAtlasKey, Vec<PyFontAtlas>)>> {
        let handle = extract_handle_from_any(id)?;
        let bevy_handle: bevy::asset::Handle<Font> = (&handle).try_into()?;
        let asset_id: AssetId<Font> = bevy_handle.id();
        let set = self.as_ref()?;
        let results: Vec<(PyFontAtlasKey, Vec<PyFontAtlas>)> = set
            .iter()
            .filter(|(key, _)| key.0 == asset_id)
            .map(|(key, atlases)| {
                (
                    key.into(),
                    atlases.iter().map(PyFontAtlas::from_bevy).collect(),
                )
            })
            .collect();
        Ok(results)
    }

    pub fn len(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.len())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_empty())
    }

    pub fn __repr__(&self) -> String {
        "FontAtlasSet(...)".to_string()
    }
}
