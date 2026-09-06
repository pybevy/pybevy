use bevy::mesh::UvChannel;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(UvChannel)]
#[pyclass(name = "UvChannel", module = "pybevy.mesh", frozen, eq, from_py_object)]
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
}

impl Default for PyUvChannel {
    fn default() -> Self {
        Self::new()
    }
}
