use bevy::image::ImageAddressMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ImageAddressMode)]
#[pyclass(name = "ImageAddressMode", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyImageAddressMode {
    ClampToEdge,
    Repeat,
    MirrorRepeat,
    ClampToBorder,
}
