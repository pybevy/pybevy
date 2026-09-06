use bevy::material::OpaqueRendererMethod;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(OpaqueRendererMethod)]
#[pyclass(
    name = "OpaqueRendererMethod",
    module = "pybevy.material",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyOpaqueRendererMethod {
    Forward,
    Deferred,
    Auto,
}
