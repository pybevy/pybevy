use bevy::{
    asset::Handle,
    image::{Image, TextureAtlasLayout, TextureAtlasSources},
    platform::collections::HashMap,
};
use pybevy_core::{PyAssetId, handle::PyHandle, materialize_asset_id, public_error};
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyDict};

use crate::texture_atlas::PyTextureAtlas;

#[pyclass(name = "TextureAtlasSources", module = "pybevy.image")]
#[derive(Debug)]
pub struct PyTextureAtlasSources {
    inner: TextureAtlasSources,
}

#[pymethods]
impl PyTextureAtlasSources {
    #[new]
    pub fn new() -> Self {
        Self {
            inner: TextureAtlasSources {
                texture_ids: HashMap::default(),
            },
        }
    }

    pub fn texture_index(&self, texture: PyHandle) -> PyResult<Option<usize>> {
        let handle: Handle<Image> = texture.try_into()?;
        Ok(self.inner.texture_index(&handle))
    }

    pub fn handle(&self, layout: PyHandle, texture: PyHandle) -> PyResult<Option<PyTextureAtlas>> {
        let layout_handle: Handle<TextureAtlasLayout> = layout.try_into()?;
        let texture_handle: Handle<Image> = texture.try_into()?;
        Ok(self
            .inner
            .handle(layout_handle, &texture_handle)
            .map(PyTextureAtlas::from))
    }

    #[getter]
    pub fn texture_ids<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let ids = PyDict::new(py);
        for (id, index) in &self.inner.texture_ids {
            ids.set_item(materialize_asset_id(py, PyAssetId::from(id))?, *index)?;
        }
        Ok(ids)
    }

    #[setter]
    pub fn set_texture_ids(&mut self, ids: &Bound<'_, PyDict>) -> PyResult<()> {
        let mut texture_ids = HashMap::default();
        for (id, index) in ids.iter() {
            let id = id.extract::<PyAssetId>()?;
            let typed = id.untyped().try_typed::<Image>().map_err(|_| {
                PyTypeError::new_err(public_error::asset_type_mismatch(
                    id.asset_type_name().unwrap_or("Unknown"),
                    "Image",
                ))
            })?;
            texture_ids.insert(typed, index.extract::<usize>()?);
        }
        self.inner.texture_ids = texture_ids;
        Ok(())
    }

    pub fn __len__(&self) -> usize {
        self.inner.texture_ids.len()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "TextureAtlasSources(texture_count={})",
            self.inner.texture_ids.len()
        )
    }
}

impl From<TextureAtlasSources> for PyTextureAtlasSources {
    fn from(inner: TextureAtlasSources) -> Self {
        Self { inner }
    }
}

impl Default for PyTextureAtlasSources {
    fn default() -> Self {
        Self::new()
    }
}

impl From<PyTextureAtlasSources> for TextureAtlasSources {
    fn from(py: PyTextureAtlasSources) -> Self {
        py.inner
    }
}
