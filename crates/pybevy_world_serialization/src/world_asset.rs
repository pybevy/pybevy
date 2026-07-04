use bevy::world_serialization::WorldAsset;
use pybevy_core::AssetStorage;
use pybevy_macros::pyasset;
use pyo3::prelude::*;

#[pyasset(WorldAsset, no_clone, bridge)]
#[pyclass(name = "WorldAsset", extends = pybevy_core::PyAsset)]
#[derive(Debug)]
pub struct PyWorldAsset {
    pub(crate) storage: AssetStorage<WorldAsset>,
}
