//! Asset bridge registrations for main crate asset types.
//!
//! This module registers asset bridges for types that are NOT yet
//! registered by feature crates. Feature crate types register their
//! own bridges in their `register_*_bridges()` functions.

use bevy::{asset::LoadedFolder, image::Image, mesh::Mesh};
use pybevy_core::registry::global_registry;
use pybevy_image::PyImage;
use pybevy_macros::asset_bridge;
use pybevy_mesh::PyMesh;
use pyo3::prelude::*;

use crate::assets::loaded_folder::PyLoadedFolder;

// Asset bridges for types still owned by main crate
// (feature crate types register their own bridges)
asset_bridge!(Mesh, PyMesh);
asset_bridge!(Image, PyImage);
asset_bridge!(LoadedFolder, PyLoadedFolder);

/// Register main crate asset bridges with the global registry.
///
/// Only registers bridges for types not handled by feature crates.
/// Feature crate types are registered by their own `register_*_bridges()`.
pub fn register_main_crate_asset_bridges() {
    global_registry::register_asset_bridge(MeshBridge);
    global_registry::register_asset_bridge(ImageBridge);
    global_registry::register_asset_bridge(LoadedFolderBridge);
}
