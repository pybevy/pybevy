use bevy::{
    asset::Handle,
    image::{Image, TextureAtlasLayout, TextureAtlasSources},
    platform::collections::HashMap,
};
use pybevy_core::handle::PyHandle;
use pyo3::prelude::*;

use crate::texture_atlas::PyTextureAtlas;

#[pyclass(name = "TextureAtlasSources")]
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

    pub fn len(&self) -> usize {
        self.inner.texture_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.texture_ids.is_empty()
    }

    pub fn indices(&self) -> Vec<usize> {
        self.inner.texture_ids.values().copied().collect()
    }

    pub fn __len__(&self) -> usize {
        self.len()
    }

    pub fn __repr__(&self) -> String {
        format!("TextureAtlasSources(texture_count={})", self.len())
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
