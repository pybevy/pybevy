use bevy::asset::UnapprovedPathMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(UnapprovedPathMode)]
#[pyclass(
    name = "UnapprovedPathMode",
    module = "pybevy.assets",
    eq,
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyUnapprovedPathMode {
    Allow,
    Deny,
    Forbid,
}
