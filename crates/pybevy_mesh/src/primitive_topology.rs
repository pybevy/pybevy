use bevy::mesh::PrimitiveTopology;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(PrimitiveTopology)]
#[pyclass(name = "PrimitiveTopology", eq, eq_int, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyPrimitiveTopology {
    PointList = 0,
    LineList = 1,
    LineStrip = 2,
    TriangleList = 3,
    TriangleStrip = 4,
}
