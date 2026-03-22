use bevy::image::ImageFilterMode;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(ImageFilterMode)]
#[pyclass(name = "ImageFilterMode", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyImageFilterMode {
    Nearest,
    Linear,
}
