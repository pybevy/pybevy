use bevy::image::ImageSamplerBorderColor;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(ImageSamplerBorderColor)]
#[pyclass(name = "ImageSamplerBorderColor", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyImageSamplerBorderColor {
    TransparentBlack,
    OpaqueBlack,
    OpaqueWhite,
    Zero,
}
