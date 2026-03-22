use bevy::pbr::OpaqueRendererMethod;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(OpaqueRendererMethod)]
#[pyclass(name = "OpaqueRendererMethod", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyOpaqueRendererMethod {
    Forward,
    Deferred,
    Auto,
}
