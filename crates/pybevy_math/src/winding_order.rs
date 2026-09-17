use bevy::math::primitives::WindingOrder;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(WindingOrder)]
#[pyclass(
    name = "WindingOrder",
    module = "pybevy.math",
    eq,
    from_py_object,
    frozen
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyWindingOrder {
    Clockwise,
    CounterClockwise,
    Invalid,
}
