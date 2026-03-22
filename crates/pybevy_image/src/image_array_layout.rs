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

    pub fn __repr__(&self) -> String {
        match self.0 {
            ImageArrayLayout::RowCount { rows } => {
                format!("ImageArrayLayout.row_count({})", rows)
            }
            ImageArrayLayout::RowHeight { pixels } => {
                format!("ImageArrayLayout.row_height({})", pixels)
            }
        }
    }
}
