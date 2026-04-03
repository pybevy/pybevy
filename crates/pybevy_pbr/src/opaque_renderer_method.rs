use bevy::pbr::OpaqueRendererMethod;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(OpaqueRendererMethod)]
#[pyclass(name = "OpaqueRendererMethod", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyOpaqueRendererMethod {
    Forward,
    Deferred,
    Auto,
}
