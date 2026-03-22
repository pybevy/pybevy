use bevy::asset::UnapprovedPathMode;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(UnapprovedPathMode, from_only)]
#[pyclass(name = "UnapprovedPathMode", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyUnapprovedPathMode {
    Allow,
    Deny,
    Forbid,
}
