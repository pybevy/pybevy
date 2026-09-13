use bevy::sprite::{BorderRect, SliceScaleMode, TextureSlicer};
use pyo3::prelude::*;

use crate::{border_rect::PyBorderRect, slice_scale_mode::PySliceScaleMode};

#[pyclass(name = "TextureSlicer", module = "pybevy.sprite", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyTextureSlicer {
    border: BorderRect,
    center_scale_mode: SliceScaleMode,
    sides_scale_mode: SliceScaleMode,
    max_corner_scale: f32,
}

#[pymethods]
impl PyTextureSlicer {
    #[new]
    #[pyo3(signature = (
        *,
        border = PyBorderRect::from_owned(BorderRect::ZERO),
        center_scale_mode = None,
        sides_scale_mode = None,
        max_corner_scale = 1.0,
    ))]
    pub fn new(
        border: PyBorderRect,
        center_scale_mode: Option<PySliceScaleMode>,
        sides_scale_mode: Option<PySliceScaleMode>,
        max_corner_scale: f32,
    ) -> PyResult<Self> {
        Ok(Self {
            border: border.try_into()?,
            center_scale_mode: center_scale_mode.map_or(SliceScaleMode::Stretch, Into::into),
            sides_scale_mode: sides_scale_mode.map_or(SliceScaleMode::Stretch, Into::into),
            max_corner_scale,
        })
    }

    #[getter]
    pub fn border(&self) -> PyBorderRect {
        PyBorderRect::from(self.border)
    }

    #[setter]
    pub fn set_border(&mut self, border: PyBorderRect) -> PyResult<()> {
        self.border = border.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn max_corner_scale(&self) -> f32 {
        self.max_corner_scale
    }

    #[setter]
    pub fn set_max_corner_scale(&mut self, max_corner_scale: f32) {
        self.max_corner_scale = max_corner_scale;
    }

    #[getter]
    pub fn center_scale_mode(&self) -> PySliceScaleMode {
        self.center_scale_mode.into()
    }

    #[setter]
    pub fn set_center_scale_mode(&mut self, mode: PySliceScaleMode) {
        self.center_scale_mode = mode.into();
    }

    #[getter]
    pub fn sides_scale_mode(&self) -> PySliceScaleMode {
        self.sides_scale_mode.into()
    }

    #[setter]
    pub fn set_sides_scale_mode(&mut self, mode: PySliceScaleMode) {
        self.sides_scale_mode = mode.into();
    }

    pub fn __repr__(&self) -> String {
        format!(
            "TextureSlicer(border={:?}, center={:?}, sides={:?}, max_corner_scale={})",
            self.border, self.center_scale_mode, self.sides_scale_mode, self.max_corner_scale
        )
    }
}

impl From<PyTextureSlicer> for TextureSlicer {
    fn from(slicer: PyTextureSlicer) -> Self {
        TextureSlicer {
            border: slicer.border,
            center_scale_mode: slicer.center_scale_mode,
            sides_scale_mode: slicer.sides_scale_mode,
            max_corner_scale: slicer.max_corner_scale,
        }
    }
}

impl From<TextureSlicer> for PyTextureSlicer {
    fn from(slicer: TextureSlicer) -> Self {
        PyTextureSlicer {
            border: slicer.border,
            center_scale_mode: slicer.center_scale_mode,
            sides_scale_mode: slicer.sides_scale_mode,
            max_corner_scale: slicer.max_corner_scale,
        }
    }
}
