use bevy::camera::primitives::CubemapLayout;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(CubemapLayout)]
#[pyclass(name = "CubemapLayout", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PyCubemapLayout {
    #[default]
    CrossVertical,
    CrossHorizontal,
    SequenceVertical,
    SequenceHorizontal,
}
