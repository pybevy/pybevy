use bevy::image::TextureAtlasLayout;
use pybevy_core::{AssetStorage, PyAsset};
use pybevy_macros::asset_storage;
use pybevy_math::{PyURect, PyUVec2};
use pyo3::prelude::*;

#[asset_storage(TextureAtlasLayout)]
#[pyclass(name = "TextureAtlasLayout", extends = PyAsset, eq)]
#[derive(Debug, PartialEq)]
pub struct PyTextureAtlasLayout {
    pub(crate) storage: AssetStorage<TextureAtlasLayout>,
}

#[pymethods]
impl PyTextureAtlasLayout {
    #[new]
    #[pyo3(signature = (size = PyUVec2::ZERO, *, textures = None))]
    pub fn new(
        size: PyUVec2,
        textures: Option<Vec<PyURect>>,
    ) -> PyResult<Py<PyTextureAtlasLayout>> {
        Python::attach(|py| {
            let layout = TextureAtlasLayout {
                size: size.into(),
                textures: textures
                    .map(|v| v.into_iter().map(|r| r.into()).collect())
                    .unwrap_or_default(),
            };
            Py::new(py, Self::from_owned(layout))
        })
    }

    #[staticmethod]
    pub fn new_empty(size: PyUVec2) -> PyResult<Py<PyTextureAtlasLayout>> {
        Python::attach(|py| {
            let layout = TextureAtlasLayout::new_empty(size.into());
            Py::new(py, Self::from_owned(layout))
        })
    }

    #[staticmethod]
    #[pyo3(signature = (tile_size, columns, rows, padding=None, offset=None))]
    pub fn from_grid(
        tile_size: PyUVec2,
        columns: u32,
        rows: u32,
        padding: Option<PyUVec2>,
        offset: Option<PyUVec2>,
    ) -> PyResult<Py<PyTextureAtlasLayout>> {
        Python::attach(|py| {
            let layout = TextureAtlasLayout::from_grid(
                tile_size.into(),
                columns,
                rows,
                padding.map(|p| p.into()),
                offset.map(|o| o.into()),
            );
            Py::new(py, Self::from_owned(layout))
        })
    }

    #[getter]
    pub fn size(&self) -> PyResult<PyUVec2> {
        Ok(self.as_ref()?.size.into())
    }

    #[getter]
    pub fn textures(&self) -> PyResult<Vec<PyURect>> {
        Ok(self
            .as_ref()?
            .textures
            .iter()
            .map(|r| (*r).into())
            .collect())
    }

    pub fn add_texture(&mut self, rect: PyURect) -> PyResult<usize> {
        Ok(self.as_mut()?.add_texture(rect.into()))
    }

    pub fn len(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.len())
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_empty())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let layout = self.as_ref()?;
        Ok(format!(
            "TextureAtlasLayout(size={:?}, texture_count={})",
            layout.size,
            layout.textures.len()
        ))
    }

    pub fn __len__(&self) -> PyResult<usize> {
        self.len()
    }
}
