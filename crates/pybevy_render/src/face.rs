use pybevy_macros::pyenum;
use pyo3::prelude::*;
use wgpu_types::Face;

#[pyenum(Face)]
#[pyclass(
    name = "Face",
    module = "pybevy.render",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyFace {
    Front,
    Back,
}
