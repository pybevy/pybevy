use bevy::sprite::{SliceScaleMode, TextureSlicer};
use pyo3::prelude::*;

use crate::{border_rect::PyBorderRect, slice_scale_mode::PySliceScaleMode};

#[pyclass(name = "TextureSlicer", frozen, eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyTextureSlicer {
    border: PyBorderRect,
    center_scale_mode: SliceScaleMode,
    sides_scale_mode: SliceScaleMode,
    max_corner_scale: f32,
}

#[pymethods]
impl PyTextureSlicer {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        border = PyBorderRect::new(pybevy_math::vec2::PyVec2::ZERO, pybevy_math::vec2::PyVec2::ZERO),
        center_tile = false,
        center_stretch_value = 1.0,
        sides_tile = false,
        sides_stretch_value = 1.0,
        max_corner_scale = 1.0,
        center_scale_mode = None,
        sides_scale_mode = None
    ))]
    pub fn new(
        border: PyBorderRect,
        center_tile: bool,
        center_stretch_value: f32,
        sides_tile: bool,
        sides_stretch_value: f32,
        max_corner_scale: f32,
        center_scale_mode: Option<PySliceScaleMode>,
        sides_scale_mode: Option<PySliceScaleMode>,
    ) -> Self {
        let center_scale_mode = if let Some(mode) = center_scale_mode {
            mode.into()
        } else if center_tile {
            SliceScaleMode::Tile {
                stretch_value: center_stretch_value,
            }
        } else {
            SliceScaleMode::Stretch
        };

        let sides_scale_mode = if let Some(mode) = sides_scale_mode {
            mode.into()
        } else if sides_tile {
            SliceScaleMode::Tile {
                stretch_value: sides_stretch_value,
            }
        } else {
            SliceScaleMode::Stretch
        };

        Self {
            border,
            center_scale_mode,
            sides_scale_mode,
            max_corner_scale,
        }
    }

    #[getter]
    pub fn border(&self) -> PyBorderRect {
        self.border.clone()
    }

    #[getter]
    pub fn max_corner_scale(&self) -> f32 {
        self.max_corner_scale
    }

    #[getter]
    pub fn center_scale_mode(&self) -> PySliceScaleMode {
        self.center_scale_mode.into()
    }

    #[getter]
    pub fn sides_scale_mode(&self) -> PySliceScaleMode {
        self.sides_scale_mode.into()
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
            border: slicer.border.into(),
            center_scale_mode: slicer.center_scale_mode.into(),
            sides_scale_mode: slicer.sides_scale_mode.into(),
            max_corner_scale: slicer.max_corner_scale,
        }
    }
}

impl From<TextureSlicer> for PyTextureSlicer {
    fn from(slicer: TextureSlicer) -> Self {
        PyTextureSlicer {
            border: slicer.border.into(),
            center_scale_mode: slicer.center_scale_mode.into(),
            sides_scale_mode: slicer.sides_scale_mode.into(),
            max_corner_scale: slicer.max_corner_scale,
        }
    }
}
