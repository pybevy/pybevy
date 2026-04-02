use bevy::{
    asset::AssetId,
    text::{Font, FontAtlasSet},
};
use pybevy_core::{ResourceStorage, extract_handle_from_any};
use pybevy_macros::resource_storage;
use pyo3::prelude::*;

use crate::font_atlas::{PyFontAtlas, PyFontAtlasKey};

#[resource_storage(FontAtlasSet, no_clone, bridge, no_mut, no_insert)]
#[pyclass(name = "FontAtlasSet", extends = pybevy_core::PyResource)]
pub struct PyFontAtlasSet {
    pub(crate) storage: ResourceStorage<FontAtlasSet>,
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
