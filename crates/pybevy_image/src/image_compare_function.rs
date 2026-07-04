use bevy::image::ImageCompareFunction;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ImageCompareFunction)]
#[pyclass(name = "ImageCompareFunction", eq, frozen, from_py_object)]
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
