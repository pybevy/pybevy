use bevy::pbr::wireframe::WireframeTopology;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(WireframeTopology)]
#[pyclass(name = "WireframeTopology", eq, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyWireframeTopology {
    Triangles,
    Quads,
}
