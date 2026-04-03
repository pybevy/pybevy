use bevy::scene::Scene;
use pybevy_core::AssetStorage;
use pybevy_macros::pyasset;
use pyo3::prelude::*;

#[pyasset(Scene, no_clone, bridge)]
#[pyclass(name = "Scene", extends = pybevy_core::PyAsset)]
#[derive(Debug)]
pub struct PyScene {
    pub(crate) storage: AssetStorage<Scene>,
}
