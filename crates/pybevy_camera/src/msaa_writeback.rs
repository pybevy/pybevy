use bevy::camera::MsaaWriteback;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(MsaaWriteback)]
#[pyclass(name = "MsaaWriteback", eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyMsaaWriteback {
    Off,
    Auto,
    Always,
}
