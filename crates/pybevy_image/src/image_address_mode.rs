use bevy::image::ImageAddressMode;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(ImageAddressMode)]
#[pyclass(name = "ImageAddressMode", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyImageAddressMode {
    ClampToEdge,
    Repeat,
    MirrorRepeat,
    ClampToBorder,
}
