use pybevy_macros::pyenum;
use pyo3::prelude::*;
use wgpu_types::Face;

#[pyenum(Face)]
#[pyclass(name = "Face", module = "pybevy.render", eq, frozen, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyFace {
    Front,
    Back,
}
