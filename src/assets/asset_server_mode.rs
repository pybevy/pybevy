use bevy::asset::AssetServerMode;
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(AssetServerMode)]
#[pyclass(name = "AssetServerMode", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyAssetServerMode {
    Unprocessed,
    Processed,
}
