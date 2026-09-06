use bevy::image::ImageAddressMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ImageAddressMode)]
#[pyclass(
    name = "ImageAddressMode",
    module = "pybevy.image",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyImageAddressMode {
    ClampToEdge,
    Repeat,
    MirrorRepeat,
    ClampToBorder,
}
