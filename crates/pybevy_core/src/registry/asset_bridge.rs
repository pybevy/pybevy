//! Asset bridge trait for runtime type dispatch
//!
//! This module provides the `AssetBridge` trait that allows feature crates
//! to register their Bevy assets without the core crate needing to import them.
//!
//! 1. Feature crate implements `AssetBridge` for each asset type
//! 2. Feature crate registers bridges via `global_registry` at init time
//! 3. Core uses bridges via runtime dispatch (no compile-time coupling)

use std::any::TypeId;

use bevy::{
    asset::{AssetPath, AssetServer, UntypedHandle},
    ecs::{component::ComponentId, world::World},
};
use pyo3::{ffi::PyTypeObject, prelude::*, types::PyType};

use crate::ValidityFlagWithMode;

/// Trait that bridges a Bevy asset to its Python wrapper.
///
/// Each feature crate implements this for its asset types. The trait provides
/// all the methods needed for:
/// - Type identification (Rust TypeId, Python type object)
/// - Getting assets from Assets<T> resource
/// - Adding new assets
/// - Removing assets
/// - Iterating assets
///
/// # Safety
///
/// The `get` and `get_mut` methods use raw pointers internally. Implementations must ensure:
/// - The validity flag is checked before dereferencing
/// - The borrowed reference doesn't outlive the system execution
pub trait AssetBridge: Send + Sync + 'static {
    /// Rust TypeId of the Bevy asset
    fn bevy_type_id(&self) -> TypeId;

    /// Python type object pointer for type matching
    ///
    /// Used for O(1) lookup in HashMap when dispatching from Python types.
    fn py_type_ptr(&self) -> *const PyTypeObject;

    /// Get Python type object
    fn py_type<'py>(&self, py: Python<'py>) -> Bound<'py, PyType>;

    /// Human-readable name for error messages
    fn name(&self) -> &'static str;

    /// Get the ComponentId of the `Assets<T>` resource in the world.
    /// Used by FilteredAccessSet to track cross-system asset access.
    fn resource_id(&self, world: &World) -> Option<ComponentId>;

    /// Register (get-or-create) the ComponentId of the `Assets<T>` resource.
    ///
    /// Unlike `resource_id`, this never returns None: it creates the id when the
    /// `Assets<T>` resource is absent so `DynamicSystem::initialize` can declare
    /// access even before the asset collection exists. The created id is
    /// TypeId-keyed and equals the one a later insertion resolves to.
    fn register_resource_id(&self, world: &mut World) -> ComponentId;

    /// Get asset from Assets resource and convert to Python object (read-only)
    ///
    /// # Arguments
    ///
    /// * `world` - World reference for accessing Assets<T> resource
    /// * `handle` - Untyped handle to the asset
    /// * `validity` - Validity flag for borrowed reference tracking
    /// * `py` - Python GIL token
    ///
    /// # Returns
    ///
    /// `Ok(Some(asset))` if asset exists, `Ok(None)` if not found.
    fn get(
        &self,
        world: &World,
        handle: &UntypedHandle,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>>;

    /// Get mutable asset from Assets resource
    ///
    /// # Arguments
    ///
    /// * `world` - Mutable world reference for accessing Assets<T> resource
    /// * `handle` - Untyped handle to the asset
    /// * `validity` - Validity flag for borrowed reference tracking
    /// * `py` - Python GIL token
    ///
    /// # Returns
    ///
    /// `Ok(Some(asset))` if asset exists, `Ok(None)` if not found.
    fn get_mut(
        &self,
        world: &mut World,
        handle: &UntypedHandle,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>>;

    /// Add new asset to Assets resource and return its handle
    ///
    /// # Arguments
    ///
    /// * `world` - Mutable world reference for accessing Assets<T> resource
    /// * `asset` - Python object to convert and add
    /// * `py` - Python GIL token
    ///
    /// # Returns
    ///
    /// Untyped handle to the newly added asset.
    fn add(&self, world: &mut World, asset: &Bound<PyAny>, py: Python) -> PyResult<UntypedHandle>;

    /// Remove asset from Assets resource
    ///
    /// # Arguments
    ///
    /// * `world` - Mutable world reference for accessing Assets<T> resource
    /// * `handle` - Untyped handle to the asset to remove
    ///
    /// # Returns
    ///
    /// `true` if asset was found and removed, `false` if not found.
    fn remove(&self, world: &mut World, handle: &UntypedHandle) -> PyResult<bool>;

    /// Remove asset from Assets resource and return it as a Python object.
    ///
    /// # Arguments
    ///
    /// * `world` - Mutable world reference for accessing Assets<T> resource
    /// * `handle` - Untyped handle to the asset to remove
    /// * `py` - Python GIL token
    ///
    /// # Returns
    ///
    /// `Ok(Some(asset))` if asset was found and removed, `Ok(None)` if not found.
    fn remove_and_return(
        &self,
        world: &mut World,
        handle: &UntypedHandle,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>>;

    /// Convert a Python input into a form acceptable by `add()`.
    ///
    /// Used for builder/factory inputs - e.g. `MeshBridge` accepts both `Mesh`
    /// instances and `MeshBuilder` / `Meshable` shapes via this hook.
    ///
    /// Returns `Ok(Some(converted))` if the input was a recognized convertible
    /// form; the converted value is then passed to `add()` in place of the
    /// original. Returns `Ok(None)` if the input is not a recognized form -
    /// the caller will then run the standard type check against the bridge's
    /// asset type.
    ///
    /// Default: no conversion. Bridges with builder support set this via
    /// `#[pyasset(T, bridge, input_converter = path::to::fn)]`.
    fn try_convert_input<'py>(
        &self,
        _asset: &Bound<'py, PyAny>,
        _py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        Ok(None)
    }

    /// Get the number of assets in the Assets<T> resource
    fn len(&self, world: &World) -> PyResult<usize>;

    /// Check if Assets<T> resource is empty
    fn is_empty(&self, world: &World) -> PyResult<bool> {
        Ok(self.len(world)? == 0)
    }

    /// Check if Assets<T> resource contains the given handle
    fn contains(&self, world: &World, handle: &UntypedHandle) -> PyResult<bool>;

    /// Iterate over all assets and return (handle, python_object) pairs
    ///
    /// Used for implementing `__iter__` on PyAssets.
    fn iter_pairs(
        &self,
        world: &World,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Vec<(UntypedHandle, Py<PyAny>)>>;

    /// Whether this asset type can be loaded from files via AssetServer.
    ///
    /// Returns `false` for asset types that are only created programmatically
    /// (e.g., `TextureAtlasLayout`, `SkinnedMeshInverseBindposes`).
    fn is_loadable(&self) -> bool {
        true
    }

    /// Load asset from file using AssetServer
    ///
    /// # Arguments
    ///
    /// * `asset_server` - Reference to Bevy's AssetServer
    /// * `path` - Asset path to load
    ///
    /// # Returns
    ///
    /// Untyped handle to the loading asset.
    fn load(&self, asset_server: &AssetServer, path: AssetPath) -> UntypedHandle;

    /// Get existing handle for an asset by path
    ///
    /// # Arguments
    ///
    /// * `asset_server` - Reference to Bevy's AssetServer
    /// * `path` - Asset path to get handle for
    ///
    /// # Returns
    ///
    /// `Some(handle)` if asset is already loaded, `None` otherwise.
    fn get_handle(&self, asset_server: &AssetServer, path: AssetPath) -> Option<UntypedHandle>;

    /// Clear programmatic assets of this type from the world.
    ///
    /// Removes assets created via `assets.add()` (no file path in AssetServer).
    /// File-loaded assets are preserved so that AssetServer handles remain valid.
    fn clear_programmatic(&self, world: &mut World, verbose: bool);
}
