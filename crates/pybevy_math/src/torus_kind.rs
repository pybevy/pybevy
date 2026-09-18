use bevy::math::primitives::TorusKind;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(TorusKind)]
#[pyclass(name = "TorusKind", module = "pybevy.math", eq, from_py_object, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyTorusKind {
    Ring,
    Horn,
    Spindle,
    Invalid,
}
