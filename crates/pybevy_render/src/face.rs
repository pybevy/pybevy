use pybevy_macros::bevy_enum;
use pyo3::prelude::*;
use wgpu_types::Face;

#[bevy_enum(Face, from_only)]
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
