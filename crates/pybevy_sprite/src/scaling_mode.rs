use bevy::sprite::SpriteScalingMode;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(SpriteScalingMode)]
#[pyclass(name = "SpriteScalingMode", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PySpriteScalingMode {
    FillCenter,
    FillStart,
    FillEnd,
    FitCenter,
    FitStart,
    FitEnd,
}
