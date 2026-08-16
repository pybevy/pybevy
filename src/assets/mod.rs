pub mod asset_event;
pub mod asset_load_failed_event;
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
#[allow(unused_imports)]
pub use pybevy_core::{NativeAsset, PyAsset, PyAssetPath};
use pybevy_core::{PyPlugin, plugin::add_plugin_if_missing};
use pyo3::prelude::*;

use crate::app::app::PyApp;

pybevy_core::register_native_system_set!(
    intern_asset_tracking_systems,
    bevy::asset::AssetTrackingSystems,
    module = "assets",
    name = "AssetTrackingSystems"
);
pybevy_core::register_native_system_set!(
    intern_asset_event_systems,
    bevy::asset::AssetEventSystems,
    module = "assets",
    name = "AssetEventSystems"
);

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
        // Under hot reload, edits to assets should land the same way edits to
        // Python do. Off otherwise so shipped apps do not carry a watcher.
        watch_for_changes_override: Some(hot_reload_enabled()),
        ..Default::default()
    }
}

/// Whether the CLI launched this process with hot reload enabled.
pub(crate) fn hot_reload_enabled() -> bool {
    std::env::var("PYBEVY_HOT_RELOAD").is_ok_and(|value| value == "1")
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
            add_plugin_if_missing(bevy_app, configured_asset_plugin());
            Ok(())
        })
    }

    /// Whether edits to files under `assets/` are picked up while the app runs.
    #[getter]
    pub fn watch_for_changes_override(&self) -> Option<bool> {
        configured_asset_plugin().watch_for_changes_override
    }
}

pub(crate) fn add_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let assets = PyModule::new(m.py(), "assets")?;
    assets.add_class::<PyAssetPlugin>()?;
    assets.add_class::<PyAsset>()?;
    assets.add_class::<pybevy_core::PyAssetIndex>()?;
    assets.add_class::<pybevy_core::PyAssetId>()?;
    pybevy_core::asset_id::register_asset_id_variants(&assets)?;
    assets.add_class::<asset_event::PyAssetEvent>()?;
    asset_event::register_asset_event_variants(&assets)?;
    assets.add_class::<asset_load_failed_event::PyAssetLoadFailedEvent>()?;
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
