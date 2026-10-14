use bevy::{image::TextureAtlasLayout, math::UVec2};
use pybevy_core::{AssetStorage, PyAsset};
use pybevy_macros::pyasset;
use pybevy_math::{urect::PyURect, uvec2::PyUVec2};
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::texture_atlas_rects::PyTextureAtlasRects;

const MAX_GRID_RECTS: u64 = 1 << 20;

fn check_grid_axis(tile: u32, count: u32, padding: u32, offset: u32) -> PyResult<()> {
    let step = u64::from(tile) + u64::from(padding);
    let total = step
        .checked_mul(u64::from(count))
        .and_then(|value| value.checked_sub(u64::from(padding)));
    let last_end = step
        .checked_mul(u64::from(count.saturating_sub(1)))
        .and_then(|value| value.checked_add(u64::from(offset)))
        .and_then(|value| value.checked_add(u64::from(tile)));
    if total.is_none_or(|value| value > u64::from(u32::MAX))
        || (count > 0 && last_end.is_none_or(|value| value > u64::from(u32::MAX)))
    {
        return Err(PyValueError::new_err(
            "texture atlas grid coordinates overflow u32",
        ));
    }
    Ok(())
}

#[pyasset(TextureAtlasLayout, bridge, not_loadable)]
#[pyclass(name = "TextureAtlasLayout", module = "pybevy.image", extends = PyAsset, eq, skip_from_py_object)]
#[derive(Debug, PartialEq)]
pub struct PyTextureAtlasLayout {
    pub(crate) storage: AssetStorage<TextureAtlasLayout>,
}

#[pymethods]
impl PyTextureAtlasLayout {
    #[new]
    #[pyo3(signature = (*, size = PyUVec2::ZERO, textures = Vec::new()))]
    pub fn new(size: PyUVec2, textures: Vec<PyURect>) -> PyResult<Py<PyTextureAtlasLayout>> {
        Python::attach(|py| {
            let layout = TextureAtlasLayout {
                size: size.try_into()?,
                textures: textures
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<PyResult<Vec<_>>>()?,
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
        let count = u64::from(columns) * u64::from(rows);
        if count > MAX_GRID_RECTS {
            return Err(PyValueError::new_err(format!(
                "texture atlas grid would create {count} rectangles; maximum is {MAX_GRID_RECTS}"
            )));
        }
        let tile_size: UVec2 = tile_size.try_into()?;
        let padding: Option<UVec2> = padding.map(TryInto::try_into).transpose()?;
        let offset: Option<UVec2> = offset.map(TryInto::try_into).transpose()?;
        let gap = padding.unwrap_or_default();
        let origin = if count == 0 {
            UVec2::ZERO
        } else {
            offset.unwrap_or_default()
        };
        let x_padding = if rows > 0 && columns > 1 { gap.x } else { 0 };
        let y_padding = if rows > 1 { gap.y } else { 0 };
        check_grid_axis(tile_size.x, columns, x_padding, origin.x)?;
        check_grid_axis(tile_size.y, rows, y_padding, origin.y)?;
        Python::attach(|py| {
            let layout = TextureAtlasLayout::from_grid(tile_size, columns, rows, padding, offset);
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

    #[setter]
    pub fn set_textures(&mut self, textures: Vec<PyURect>) -> PyResult<()> {
        let textures = textures
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PyResult<Vec<_>>>()?;
        self.as_mut()?.textures = textures;
        Ok(())
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
