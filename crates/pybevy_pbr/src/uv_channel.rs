use bevy::pbr::UvChannel;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(UvChannel)]
#[pyclass(name = "UvChannel", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyUvChannel {
    Uv0,
    Uv1,
}

#[pymethods]
impl PyUvChannel {
    #[new]
    pub fn new() -> Self {
        PyUvChannel::Uv0
    }
    #[classattr]
    pub const UV0: Self = PyUvChannel::Uv0;
    #[classattr]
    pub const UV1: Self = PyUvChannel::Uv1;
}

impl Default for PyUvChannel {
    fn default() -> Self {
        Self::new()
    }
}
