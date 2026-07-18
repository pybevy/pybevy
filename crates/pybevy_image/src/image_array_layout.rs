use bevy::image::ImageArrayLayout;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ImageArrayLayout, manual)]
#[pyclass(name = "ImageArrayLayout", frozen, from_py_object)]
#[derive(Debug, Clone, Copy)]
pub enum PyImageArrayLayout {
    RowCount {
        rows: u32,
    },
    RowHeight {
        pixels: u32,
    },
    GridCount {
        columns: u32,
        rows: u32,
    },
    GridSize {
        tile_width_pixels: u32,
        tile_height_pixels: u32,
    },
}

impl From<ImageArrayLayout> for PyImageArrayLayout {
    fn from(value: ImageArrayLayout) -> Self {
        match value {
            ImageArrayLayout::RowCount { rows } => Self::RowCount { rows },
            ImageArrayLayout::RowHeight { pixels } => Self::RowHeight { pixels },
            ImageArrayLayout::GridCount { columns, rows } => Self::GridCount { columns, rows },
            ImageArrayLayout::GridSize {
                tile_width_pixels,
                tile_height_pixels,
            } => Self::GridSize {
                tile_width_pixels,
                tile_height_pixels,
            },
        }
    }
}

impl From<PyImageArrayLayout> for ImageArrayLayout {
    fn from(value: PyImageArrayLayout) -> Self {
        match value {
            PyImageArrayLayout::RowCount { rows } => Self::RowCount { rows },
            PyImageArrayLayout::RowHeight { pixels } => Self::RowHeight { pixels },
            PyImageArrayLayout::GridCount { columns, rows } => Self::GridCount { columns, rows },
            PyImageArrayLayout::GridSize {
                tile_width_pixels,
                tile_height_pixels,
            } => Self::GridSize {
                tile_width_pixels,
                tile_height_pixels,
            },
        }
    }
}

#[pymethods]
impl PyImageArrayLayout {
    pub fn __repr__(&self) -> String {
        match self {
            Self::RowCount { rows } => format!("ImageArrayLayout.RowCount(rows={rows})"),
            Self::RowHeight { pixels } => {
                format!("ImageArrayLayout.RowHeight(pixels={pixels})")
            }
            Self::GridCount { columns, rows } => {
                format!("ImageArrayLayout.GridCount(columns={columns}, rows={rows})")
            }
            Self::GridSize {
                tile_width_pixels,
                tile_height_pixels,
            } => format!(
                "ImageArrayLayout.GridSize(tile_width_pixels={tile_width_pixels}, tile_height_pixels={tile_height_pixels})"
            ),
        }
    }
}
