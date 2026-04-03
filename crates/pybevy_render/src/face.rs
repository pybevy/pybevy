use pybevy_macros::pyenum;
use pyo3::prelude::*;
use wgpu_types::Face;

#[pyenum(Face)]
#[pyclass(name = "Face", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyFace {
    Front,
    Back,
}

#[pymethods]
impl PyFace {
    #[classattr]
    pub const FRONT: Self = PyFace::Front;
    #[classattr]
    pub const BACK: Self = PyFace::Back;
}
