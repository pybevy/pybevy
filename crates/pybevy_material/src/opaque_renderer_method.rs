use bevy::material::OpaqueRendererMethod;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(OpaqueRendererMethod)]
#[pyclass(name = "OpaqueRendererMethod", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyOpaqueRendererMethod {
    Forward,
    Deferred,
    Auto,
}

#[pymethods]
impl PyOpaqueRendererMethod {
    #[classattr]
    pub const FORWARD: Self = PyOpaqueRendererMethod::Forward;
    #[classattr]
    pub const DEFERRED: Self = PyOpaqueRendererMethod::Deferred;
    #[classattr]
    pub const AUTO: Self = PyOpaqueRendererMethod::Auto;
}
