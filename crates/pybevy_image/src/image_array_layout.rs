use bevy::image::ImageArrayLayout;
use pyo3::prelude::*;

#[pyclass(name = "ImageArrayLayout", frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PyImageArrayLayout(pub(crate) ImageArrayLayout);

impl From<ImageArrayLayout> for PyImageArrayLayout {
    fn from(val: ImageArrayLayout) -> Self {
        PyImageArrayLayout(val)
    }
}

impl From<PyImageArrayLayout> for ImageArrayLayout {
    fn from(val: PyImageArrayLayout) -> Self {
        val.0
    }
}

#[pymethods]
impl PyImageArrayLayout {
    #[staticmethod]
    pub fn row_count(rows: u32) -> Self {
        PyImageArrayLayout(ImageArrayLayout::RowCount { rows })
    }

    #[staticmethod]
    pub fn row_height(pixels: u32) -> Self {
        PyImageArrayLayout(ImageArrayLayout::RowHeight { pixels })
    }

    #[staticmethod]
    pub fn grid_count(columns: u32, rows: u32) -> Self {
        PyImageArrayLayout(ImageArrayLayout::GridCount { columns, rows })
    }

    #[staticmethod]
    pub fn grid_size(tile_width_pixels: u32, tile_height_pixels: u32) -> Self {
        PyImageArrayLayout(ImageArrayLayout::GridSize {
            tile_width_pixels,
            tile_height_pixels,
        })
    }

    pub fn __repr__(&self) -> String {
        match self.0 {
            ImageArrayLayout::RowCount { rows } => {
                format!("ImageArrayLayout.row_count({})", rows)
            }
            ImageArrayLayout::RowHeight { pixels } => {
                format!("ImageArrayLayout.row_height({})", pixels)
            }
            ImageArrayLayout::GridCount { columns, rows } => {
                format!("ImageArrayLayout.grid_count({}, {})", columns, rows)
            }
            ImageArrayLayout::GridSize {
                tile_width_pixels,
                tile_height_pixels,
            } => {
                format!(
                    "ImageArrayLayout.grid_size({}, {})",
                    tile_width_pixels, tile_height_pixels
                )
            }
        }
    }
}
