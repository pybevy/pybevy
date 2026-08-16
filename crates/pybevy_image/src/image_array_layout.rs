use bevy::image::ImageArrayLayout;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ImageArrayLayout)]
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
