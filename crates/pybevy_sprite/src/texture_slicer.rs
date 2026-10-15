use bevy::sprite::{BorderRect, TextureSlicer};
use pybevy_core::{BorrowableStorage, FieldStorage, FieldStorageInner};
use pyo3::prelude::*;

use crate::{border_rect::PyBorderRect, slice_scale_mode::PySliceScaleMode};

#[pyclass(name = "TextureSlicer", module = "pybevy.sprite", eq, from_py_object)]
#[derive(Debug)]
pub struct PyTextureSlicer {
    storage: FieldStorage<TextureSlicer>,
}

impl Clone for PyTextureSlicer {
    // By-value copies, such as enum payloads, are frozen snapshots.
    fn clone(&self) -> Self {
        let storage = match &self.storage.inner {
            FieldStorageInner::Owned { data, .. } | FieldStorageInner::OwnedReadOnly { data } => {
                FieldStorage::snapshot(&**data)
            }
            _ => self.storage.clone(),
        };
        Self { storage }
    }
}

impl PartialEq for PyTextureSlicer {
    fn eq(&self, other: &Self) -> bool {
        match (self.storage.as_ref(), other.storage.as_ref()) {
            (Ok(left), Ok(right)) => *left == *right,
            _ => false,
        }
    }
}

#[pymethods]
impl PyTextureSlicer {
    #[new]
    #[pyo3(signature = (
        *,
        border = PyBorderRect::from_owned(BorderRect::ZERO),
        center_scale_mode = PySliceScaleMode::Stretch(),
        sides_scale_mode = PySliceScaleMode::Stretch(),
        max_corner_scale = 1.0,
    ))]
    pub fn new(
        border: PyBorderRect,
        center_scale_mode: PySliceScaleMode,
        sides_scale_mode: PySliceScaleMode,
        max_corner_scale: f32,
    ) -> PyResult<Self> {
        Ok(Self::from(TextureSlicer {
            border: border.try_into()?,
            center_scale_mode: center_scale_mode.into(),
            sides_scale_mode: sides_scale_mode.into(),
            max_corner_scale,
        }))
    }

    #[getter]
    pub fn border(&self) -> PyResult<PyBorderRect> {
        Ok(self.storage.borrow_field_as(|slicer| &slicer.border)?)
    }

    #[setter]
    pub fn set_border(&mut self, border: PyBorderRect) -> PyResult<()> {
        self.storage.as_mut()?.border = border.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn max_corner_scale(&self) -> PyResult<f32> {
        Ok(self.storage.as_ref()?.max_corner_scale)
    }

    #[setter]
    pub fn set_max_corner_scale(&mut self, max_corner_scale: f32) -> PyResult<()> {
        self.storage.as_mut()?.max_corner_scale = max_corner_scale;
        Ok(())
    }

    #[getter]
    pub fn center_scale_mode(&self) -> PyResult<PySliceScaleMode> {
        Ok(self.storage.as_ref()?.center_scale_mode.into())
    }

    #[setter]
    pub fn set_center_scale_mode(&mut self, mode: PySliceScaleMode) -> PyResult<()> {
        self.storage.as_mut()?.center_scale_mode = mode.into();
        Ok(())
    }

    #[getter]
    pub fn sides_scale_mode(&self) -> PyResult<PySliceScaleMode> {
        Ok(self.storage.as_ref()?.sides_scale_mode.into())
    }

    #[setter]
    pub fn set_sides_scale_mode(&mut self, mode: PySliceScaleMode) -> PyResult<()> {
        self.storage.as_mut()?.sides_scale_mode = mode.into();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let slicer = self.storage.as_ref()?;
        Ok(format!(
            "TextureSlicer(border={}, center_scale_mode={}, sides_scale_mode={}, \
             max_corner_scale={})",
            PyBorderRect::from(slicer.border).__repr__()?,
            PySliceScaleMode::from(slicer.center_scale_mode).__repr__(),
            PySliceScaleMode::from(slicer.sides_scale_mode).__repr__(),
            slicer.max_corner_scale,
        ))
    }
}

impl TryFrom<PyTextureSlicer> for TextureSlicer {
    type Error = PyErr;

    fn try_from(slicer: PyTextureSlicer) -> PyResult<Self> {
        Ok(slicer.storage.get()?)
    }
}

impl From<TextureSlicer> for PyTextureSlicer {
    fn from(slicer: TextureSlicer) -> Self {
        Self {
            storage: FieldStorage::owned(slicer),
        }
    }
}
