use bevy::mesh::PrimitiveTopology;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(PrimitiveTopology)]
#[pyclass(name = "PrimitiveTopology", eq, eq_int, frozen, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyPrimitiveTopology {
    PointList = 0,
    LineList = 1,
    LineStrip = 2,
    TriangleList = 3,
    TriangleStrip = 4,
}
