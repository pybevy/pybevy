use bevy::pbr::OpaqueRendererMethod;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(OpaqueRendererMethod)]
#[pyclass(name = "OpaqueRenderMethod", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyOpaqueRenderMethod {
    Forward,
    Deferred,
    Auto,
}

#[pymethods]
impl PyOpaqueRenderMethod {
    #[classattr]
    pub const FORWARD: Self = PyOpaqueRenderMethod::Forward;
    #[classattr]
    pub const DEFERRED: Self = PyOpaqueRenderMethod::Deferred;
    #[classattr]
    pub const AUTO: Self = PyOpaqueRenderMethod::Auto;
}
