use bevy::asset::AssetServerMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(AssetServerMode)]
#[pyclass(name = "AssetServerMode", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyAssetServerMode {
    Unprocessed,
    Processed,
}
