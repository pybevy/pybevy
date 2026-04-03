use bevy::asset::UnapprovedPathMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(UnapprovedPathMode)]
#[pyclass(name = "UnapprovedPathMode", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyUnapprovedPathMode {
    Allow,
    Deny,
    Forbid,
}
