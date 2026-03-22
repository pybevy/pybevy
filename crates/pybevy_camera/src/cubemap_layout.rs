use bevy::camera::primitives::CubemapLayout;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(CubemapLayout)]
#[pyclass(name = "CubemapLayout", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PyCubemapLayout {
    #[default]
    CrossVertical,
    CrossHorizontal,
    SequenceVertical,
    SequenceHorizontal,
}
