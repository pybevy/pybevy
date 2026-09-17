use naga::ShaderStage;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(ShaderStage)]
#[pyclass(
    name = "ShaderStage",
    module = "pybevy.shader",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyShaderStage {
    Vertex,
    Task,
    Mesh,
    Fragment,
    Compute,
    RayGeneration,
    Miss,
    AnyHit,
    ClosestHit,
}
