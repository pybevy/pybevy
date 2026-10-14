use bevy::gltf::GltfPrimitive;
use pybevy_core::FieldStorage;
use pyo3::prelude::*;

use crate::assets::PyGltfPrimitive;

#[pyclass(name = "_GltfPrimitives", module = "pybevy.gltf", skip_from_py_object)]
#[derive(Clone)]
pub struct PyGltfPrimitives {
    storage: FieldStorage<Vec<GltfPrimitive>>,
}

pybevy_core::impl_live_asset_sequence!(
    PyGltfPrimitives,
    "_GltfPrimitives",
    Vec<GltfPrimitive>,
    GltfPrimitive,
    PyGltfPrimitive
);
