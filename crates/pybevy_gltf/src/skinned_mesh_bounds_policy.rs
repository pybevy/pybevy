use bevy::gltf::GltfSkinnedMeshBoundsPolicy;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(GltfSkinnedMeshBoundsPolicy)]
#[pyclass(
    name = "GltfSkinnedMeshBoundsPolicy",
    module = "pybevy.gltf",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyGltfSkinnedMeshBoundsPolicy {
    BindPose,
    Dynamic,
    NoFrustumCulling,
}
