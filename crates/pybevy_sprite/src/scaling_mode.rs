use bevy::sprite::SpriteScalingMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(SpriteScalingMode)]
#[pyclass(
    name = "SpriteScalingMode",
    module = "pybevy.sprite",
    eq,
    from_py_object,
    frozen
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PySpriteScalingMode {
    FillCenter,
    FillStart,
    FillEnd,
    FitCenter,
    FitStart,
    FitEnd,
}
