use bevy::image::ImageCompareFunction;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(ImageCompareFunction)]
#[pyclass(name = "ImageCompareFunction", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyImageCompareFunction {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}
