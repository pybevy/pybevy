use bevy::sprite::SpriteScalingMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(SpriteScalingMode)]
#[pyclass(name = "SpriteScalingMode", eq, from_py_object, frozen, hash)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PySpriteScalingMode {
    FillCenter,
    FillStart,
    FillEnd,
    FitCenter,
    FitStart,
    FitEnd,
}
