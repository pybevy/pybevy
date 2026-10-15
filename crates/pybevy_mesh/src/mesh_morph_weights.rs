use bevy::{ecs::entity::Entity, mesh::morph::MeshMorphWeights};
use pybevy_core::{ComponentStorage, PyEntity, PyFloatLiveList};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(MeshMorphWeights, component)]
#[pyclass(name = "MeshMorphWeights", module = "pybevy.mesh")]
pub enum PyMeshMorphWeights {
    Value {
        #[py_type(PyFloatLiveList)]
        #[py_try_into]
        #[py_borrow]
        #[py_set]
        weights: Vec<f32>,
    },
    #[py_bevy(tuple)]
    Reference {
        #[py_type(PyEntity)]
        #[py_set]
        value: Entity,
    },
}
