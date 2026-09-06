use bevy::asset::AssetServerMode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(AssetServerMode)]
#[pyclass(
    name = "AssetServerMode",
    module = "pybevy.assets",
    eq,
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyAssetServerMode {
    Unprocessed,
    Processed,
}
