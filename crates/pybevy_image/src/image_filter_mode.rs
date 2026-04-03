use bevy::image::ImageFilterMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ImageFilterMode)]
#[pyclass(name = "ImageFilterMode", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyImageFilterMode {
    Nearest,
    Linear,
}
