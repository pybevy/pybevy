use bevy::{
    animation::AnimationClip,
    asset::Handle,
    gltf::{GltfMaterial, GltfMesh, GltfNode, GltfSkin},
    world_serialization::WorldAsset,
};
use pybevy_core::{FieldStorage, PyHandle};
use pyo3::{prelude::*, types::PyList};

#[pyclass(
    name = "_GltfSceneHandles",
    module = "pybevy.gltf",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyGltfSceneHandles {
    storage: FieldStorage<Vec<Handle<WorldAsset>>>,
}

pybevy_core::impl_live_value_list!(
    PyGltfSceneHandles,
    "_GltfSceneHandles",
    Vec<Handle<WorldAsset>>,
    Handle<WorldAsset>,
    PyHandle
);

#[pyclass(name = "_GltfMeshHandles", module = "pybevy.gltf", skip_from_py_object)]
#[derive(Clone)]
pub struct PyGltfMeshHandles {
    storage: FieldStorage<Vec<Handle<GltfMesh>>>,
}

pybevy_core::impl_live_value_list!(
    PyGltfMeshHandles,
    "_GltfMeshHandles",
    Vec<Handle<GltfMesh>>,
    Handle<GltfMesh>,
    PyHandle
);

#[pyclass(
    name = "_GltfMaterialHandles",
    module = "pybevy.gltf",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyGltfMaterialHandles {
    storage: FieldStorage<Vec<Handle<GltfMaterial>>>,
}

pybevy_core::impl_live_value_list!(
    PyGltfMaterialHandles,
    "_GltfMaterialHandles",
    Vec<Handle<GltfMaterial>>,
    Handle<GltfMaterial>,
    PyHandle
);

#[pyclass(name = "_GltfNodeHandles", module = "pybevy.gltf", skip_from_py_object)]
#[derive(Clone)]
pub struct PyGltfNodeHandles {
    storage: FieldStorage<Vec<Handle<GltfNode>>>,
}

pybevy_core::impl_live_value_list!(
    PyGltfNodeHandles,
    "_GltfNodeHandles",
    Vec<Handle<GltfNode>>,
    Handle<GltfNode>,
    PyHandle
);

#[pyclass(name = "_GltfSkinHandles", module = "pybevy.gltf", skip_from_py_object)]
#[derive(Clone)]
pub struct PyGltfSkinHandles {
    storage: FieldStorage<Vec<Handle<GltfSkin>>>,
}

pybevy_core::impl_live_value_list!(
    PyGltfSkinHandles,
    "_GltfSkinHandles",
    Vec<Handle<GltfSkin>>,
    Handle<GltfSkin>,
    PyHandle
);

#[pyclass(
    name = "_AnimationClipHandles",
    module = "pybevy.gltf",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyAnimationClipHandles {
    storage: FieldStorage<Vec<Handle<AnimationClip>>>,
}

pybevy_core::impl_live_value_list!(
    PyAnimationClipHandles,
    "_AnimationClipHandles",
    Vec<Handle<AnimationClip>>,
    Handle<AnimationClip>,
    PyHandle
);
