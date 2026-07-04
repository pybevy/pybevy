use bevy::world_serialization::DynamicWorld;
use pybevy_core::AssetStorage;
use pybevy_macros::pyasset;
use pyo3::prelude::*;

#[pyasset(DynamicWorld, no_clone, bridge)]
#[pyclass(name = "DynamicWorld", extends = pybevy_core::PyAsset)]
pub struct PyDynamicWorld {
    pub(crate) storage: AssetStorage<DynamicWorld>,
}
