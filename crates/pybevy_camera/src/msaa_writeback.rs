use bevy::camera::MsaaWriteback;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(MsaaWriteback)]
#[pyclass(
    name = "MsaaWriteback",
    module = "pybevy.camera",
    eq,
    from_py_object,
    frozen,
    hash
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyMsaaWriteback {
    Off,
    Auto,
    Always,
}
