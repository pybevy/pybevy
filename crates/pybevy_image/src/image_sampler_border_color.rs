use bevy::image::ImageSamplerBorderColor;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ImageSamplerBorderColor)]
#[pyclass(
    name = "ImageSamplerBorderColor",
    module = "pybevy.image",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyImageSamplerBorderColor {
    TransparentBlack,
    OpaqueBlack,
    OpaqueWhite,
    Zero,
}
