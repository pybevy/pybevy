use bevy::scene::DynamicScene;
use pybevy_core::AssetStorage;
use pybevy_macros::asset_storage;
use pyo3::prelude::*;

#[asset_storage(DynamicScene, no_clone, bridge)]
#[pyclass(name = "DynamicScene", extends = pybevy_core::PyAsset)]
pub struct PyDynamicScene {
    pub(crate) storage: AssetStorage<DynamicScene>,
}
