use bevy::image::ImageSamplerBorderColor;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ImageSamplerBorderColor)]
#[pyclass(name = "ImageSamplerBorderColor", eq, frozen, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyImageSamplerBorderColor {
    TransparentBlack,
    OpaqueBlack,
    OpaqueWhite,
    Zero,
}
