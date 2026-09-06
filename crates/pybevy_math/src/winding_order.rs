use bevy::math::primitives::WindingOrder;
use pyo3::prelude::*;

#[pyclass(name = "WindingOrder", module = "pybevy.math", eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyWindingOrder {
    Clockwise,
    CounterClockwise,
    Invalid,
}

impl From<WindingOrder> for PyWindingOrder {
    fn from(order: WindingOrder) -> Self {
        match order {
            WindingOrder::Clockwise => PyWindingOrder::Clockwise,
            WindingOrder::CounterClockwise => PyWindingOrder::CounterClockwise,
            WindingOrder::Invalid => PyWindingOrder::Invalid,
        }
    }
}

#[pymethods]
impl PyWindingOrder {
    fn __repr__(&self) -> String {
        match self {
            PyWindingOrder::Clockwise => "WindingOrder.Clockwise".to_string(),
            PyWindingOrder::CounterClockwise => "WindingOrder.CounterClockwise".to_string(),
            PyWindingOrder::Invalid => "WindingOrder.Invalid".to_string(),
        }
    }
}
