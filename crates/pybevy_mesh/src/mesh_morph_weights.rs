use bevy::{ecs::entity::Entity, mesh::morph::MeshMorphWeights};
use pybevy_core::{ComponentStorage, PyEntity};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(MeshMorphWeights, component)]
#[pyclass(name = "MeshMorphWeights", module = "pybevy.mesh")]
pub enum PyMeshMorphWeights {
    Value {
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
