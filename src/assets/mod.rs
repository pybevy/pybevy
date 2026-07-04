pub mod asset_event;
pub mod asset_server;
pub mod asset_server_mode;
pub mod asset_type;
pub mod assets;
pub mod dependency_load_state;
pub mod load_state;
pub mod loaded_folder;
pub mod recursive_dependency_load_state;
pub mod unapproved_path_mode;

// Re-export from pybevy_core
use std::{env::current_dir, path::PathBuf};

use bevy::{asset::AssetPlugin, log::warn};
use pybevy_core::PyPlugin;
#[allow(unused_imports)]
pub use pybevy_core::{NativeAsset, PyAsset, PyAssetPath};
use pyo3::prelude::*;

use crate::app::app::PyApp;

/// Returns a configured AssetPlugin that uses the ./assets directory
/// relative to the current working directory.
///
/// This is used by both PyAssetPlugin and PyDefaultPlugins to ensure
/// consistent asset path configuration across the codebase.
///
/// NOTE: This workaround is intentional and necessary for Python scripts.
/// Bevy's default asset path ("assets") is relative to the executable, which
/// for Python would be the interpreter itself (e.g., /usr/bin/python3).
/// We use CWD instead, which is typically where `python script.py` is run from.
pub(crate) fn configured_asset_plugin() -> AssetPlugin {
    let assets_path: PathBuf = current_dir()
        .unwrap_or_else(|e| {
            warn!(
                "Could not determine current directory ({}), using '.' as fallback",
                e
            );
            PathBuf::from(".")
        })
        .join("assets");

    AssetPlugin {
        file_path: assets_path.to_string_lossy().to_string(),
        ..Default::default()
    }
}

#[pyclass(name = "AssetPlugin", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyAssetPlugin;

#[pymethods]
impl PyAssetPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyAssetPlugin, PyPlugin).into()
    }

    pub fn build(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        app.borrow().with_bevy_app(|bevy_app| {
            bevy_app.add_plugins(configured_asset_plugin());
            Ok(())
        })
    }
}

pub(crate) fn add_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let assets = PyModule::new(m.py(), "assets")?;
    assets.add_class::<PyAssetPlugin>()?;
    assets.add_class::<PyAsset>()?;
    assets.add_class::<asset_event::PyAssetEvent>()?;
    assets.add_class::<asset_event::PyAssetEventType>()?;
    assets.add_class::<asset_server::PyAssetServer>()?;
    assets.add_class::<asset_type::PyAssetTypeParam>()?;
    assets.add_class::<assets::PyAssets>()?;
    assets.add_class::<assets::PyAssetIter>()?;
    assets.add_class::<pybevy_core::handle::PyHandle>()?;
    assets.add_class::<load_state::PyLoadState>()?;
    assets.add_class::<dependency_load_state::PyDependencyLoadState>()?;
    assets.add_class::<recursive_dependency_load_state::PyRecursiveDependencyLoadState>()?;
    assets.add_class::<asset_server_mode::PyAssetServerMode>()?;
    assets.add_class::<unapproved_path_mode::PyUnapprovedPathMode>()?;
    assets.add_class::<loaded_folder::PyLoadedFolder>()?;
    assets.add_class::<PyAssetPath>()?;
    m.add_submodule(&assets)
}
