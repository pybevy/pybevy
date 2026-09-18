use bevy::image::ImageArrayLayout;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ImageArrayLayout)]
#[pyclass(
    name = "ImageArrayLayout",
    module = "pybevy.image",
    frozen,
    from_py_object,
    eq,
    hash
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyImageArrayLayout {
    #[pyo3(constructor = (*, rows))]
    RowCount { rows: u32 },
    #[pyo3(constructor = (*, pixels))]
    RowHeight { pixels: u32 },
    #[pyo3(constructor = (*, columns, rows))]
    GridCount { columns: u32, rows: u32 },
    #[pyo3(constructor = (*, tile_width_pixels, tile_height_pixels))]
    GridSize {
        tile_width_pixels: u32,
        tile_height_pixels: u32,
    },
}
