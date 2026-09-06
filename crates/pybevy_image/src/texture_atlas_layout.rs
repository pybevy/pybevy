use bevy::image::TextureAtlasLayout;
use pybevy_core::{AssetStorage, PyAsset};
use pybevy_macros::pyasset;
use pybevy_math::{urect::PyURect, uvec2::PyUVec2};
use pyo3::prelude::*;

use crate::texture_atlas_rects::PyTextureAtlasRects;

#[pyasset(TextureAtlasLayout, bridge, not_loadable)]
#[pyclass(name = "TextureAtlasLayout", module = "pybevy.image", extends = PyAsset, eq, skip_from_py_object)]
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
                size: size.try_into()?,
                textures: match textures {
                    Some(v) => v
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<PyResult<Vec<_>>>()?,
                    None => Vec::new(),
                },
            };
            Py::new(py, Self::from_owned(layout))
        })
    }

    #[staticmethod]
    pub fn new_empty(size: PyUVec2) -> PyResult<Py<PyTextureAtlasLayout>> {
        Python::attach(|py| {
            let layout = TextureAtlasLayout::new_empty(size.try_into()?);
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
                tile_size.try_into()?,
                columns,
                rows,
                padding.map(TryInto::try_into).transpose()?,
                offset.map(TryInto::try_into).transpose()?,
            );
            Py::new(py, Self::from_owned(layout))
        })
    }

    #[getter]
    pub fn size(&self) -> PyResult<PyUVec2> {
        Ok(self
            .storage
            .borrow_field_as(|layout| &layout.size, |layout| &mut layout.size)?)
    }

    #[setter]
    pub fn set_size(&mut self, size: PyUVec2) -> PyResult<()> {
        let size = size.try_into()?;
        self.as_mut()?.size = size;
        Ok(())
    }

    #[getter]
    pub fn textures(&self) -> PyResult<PyTextureAtlasRects> {
        Ok(self
            .storage
            .borrow_field_as(|layout| &layout.textures, |layout| &mut layout.textures)?)
    }

    pub fn add_texture(&mut self, rect: PyURect) -> PyResult<usize> {
        let rect = rect.try_into()?;
        Ok(self.as_mut()?.add_texture(rect))
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
