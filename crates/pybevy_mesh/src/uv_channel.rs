use bevy::mesh::UvChannel;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(UvChannel)]
#[pyclass(name = "UvChannel", module = "pybevy.mesh", frozen, eq, from_py_object)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum PyUvChannel {
    #[default]
    Uv0,
    Uv1,
}
