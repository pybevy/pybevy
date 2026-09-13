//! Asset bridge trait for runtime type dispatch
//!
//! This module provides the `AssetBridge` trait that allows feature crates
//! to register their Bevy assets without the core crate needing to import them.
//!
//! 1. Feature crate implements `AssetBridge` for each asset type
//! 2. Feature crate registers bridges via `global_registry` at init time
//! 3. Core uses bridges via runtime dispatch (no compile-time coupling)

use std::any::{Any, TypeId};

use bevy::{
    asset::{AssetPath, AssetServer, UntypedAssetId, UntypedHandle},
    ecs::{
        component::ComponentId,
        world::{World, unsafe_world_cell::UnsafeWorldCell},
    },
};
use pyo3::{ffi::PyTypeObject, prelude::*, types::PyType};

use crate::{AssetBorrowCounter, ValidityFlagWithMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetEventRecord {
    Added { id: UntypedAssetId },
    Modified { id: UntypedAssetId },
    Removed { id: UntypedAssetId },
    Unused { id: UntypedAssetId },
    LoadedWithDependencies { id: UntypedAssetId },
}

/// A failed asset load, flattened for the type-erased bridge boundary.
///
/// `error` is `AssetLoadError`'s `Display` text: its variants carry loader
/// internals with no Python value model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetLoadFailedRecord {
    pub id: UntypedAssetId,
    pub path: AssetPath<'static>,
    pub error: String,
}

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

    /// Register the message-buffer component ID for `AssetEvent<T>`.
    fn register_event_resource_id(&self, world: &mut World) -> ComponentId;

    /// Read new events through the reader's type-erased persistent cursor.
    fn read_events(
        &self,
        world: &World,
        cursor: &mut Option<Box<dyn Any + Send + Sync>>,
    ) -> Vec<AssetEventRecord>;

    fn clear_events(&self, world: &mut World);

    fn events_is_empty(&self, world: &World) -> bool;

    fn event_count(&self, world: &World) -> usize;

    /// Register the message-buffer component ID for `AssetLoadFailedEvent<T>`.
    fn register_load_failed_resource_id(&self, world: &mut World) -> ComponentId;

    /// Read new load failures through the reader's type-erased persistent cursor.
    fn read_load_failed_events(
        &self,
        world: &World,
        cursor: &mut Option<Box<dyn Any + Send + Sync>>,
    ) -> Vec<AssetLoadFailedRecord>;

    fn clear_load_failed_events(&self, world: &mut World);

    fn load_failed_events_is_empty(&self, world: &World) -> bool;

    fn load_failed_event_count(&self, world: &World) -> usize;

    /// Get asset from Assets resource and convert to Python object (read-only)
    fn get(
        &self,
        world: &World,
        id: UntypedAssetId,
        validity: ValidityFlagWithMode,
        borrow_counter: AssetBorrowCounter,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>>;

    /// Get mutable asset from Assets resource
    fn get_mut(
        &self,
        world: UnsafeWorldCell<'_>,
        id: UntypedAssetId,
        validity: ValidityFlagWithMode,
        borrow_counter: AssetBorrowCounter,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>>;

    /// Add new asset to Assets resource and return its handle
    fn add(&self, world: &mut World, asset: &Bound<PyAny>, py: Python) -> PyResult<UntypedHandle>;

    /// Remove asset from Assets resource
    fn remove(&self, world: &mut World, id: UntypedAssetId) -> PyResult<bool>;

    /// Remove asset from Assets resource and return it as a Python object.
    fn remove_and_return(
        &self,
        world: &mut World,
        id: UntypedAssetId,
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

    /// Check if Assets<T> contains the given asset ID.
    fn contains(&self, world: &World, id: UntypedAssetId) -> PyResult<bool>;

    /// Iterate over all assets and return (ID, Python object) pairs.
    ///
    /// Used for implementing `__iter__` on PyAssets.
    fn iter_pairs(
        &self,
        world: &World,
        validity: ValidityFlagWithMode,
        borrow_counter: AssetBorrowCounter,
        py: Python,
    ) -> PyResult<Vec<(UntypedAssetId, Py<PyAny>)>>;

    /// Whether this asset type can be loaded from files via AssetServer.
    ///
    /// Returns `false` for asset types that are only created programmatically
    /// (e.g., `TextureAtlasLayout`, `SkinnedMeshInverseBindposes`).
    fn is_loadable(&self) -> bool {
        true
    }

    /// Load asset from file using AssetServer
    fn load(&self, asset_server: &AssetServer, path: AssetPath) -> UntypedHandle;

    /// Get existing handle for an asset by path
    fn get_handle(&self, asset_server: &AssetServer, path: AssetPath) -> Option<UntypedHandle>;

    /// Clear programmatic assets of this type from the world.
    ///
    /// Removes assets created via `assets.add()` (no file path in AssetServer).
    /// File-loaded assets are preserved so that AssetServer handles remain valid.
    fn clear_programmatic(&self, world: &mut World, verbose: bool);
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::{any::TypeId, sync::Arc};

    use bevy::{
        asset::{Asset, AssetPath, AssetServer, Handle, UntypedAssetId, UntypedHandle},
        ecs::{
            component::ComponentId,
            world::{World, unsafe_world_cell::UnsafeWorldCell},
        },
        reflect::TypePath,
    };
    use pyo3::{exceptions::PyRuntimeError, ffi::PyTypeObject, prelude::*, types::PyType};

    use super::*;
    use crate::registry::global_registry::{
        contains_asset_py_type, get_asset_bridge_by_name, get_asset_bridge_by_py_type,
        get_asset_bridge_by_type_id, register_asset_bridge_arc,
    };

    #[derive(Asset, TypePath)]
    struct FakeAssetType;

    #[derive(Asset, TypePath)]
    struct FakeAssetType2;

    // Opaque non-null keys, never dereferenced (same pattern as the component-registry tests).
    const FAKE_PTR: *const PyTypeObject = 0x2001_usize as *const PyTypeObject;
    const FAKE_PTR2: *const PyTypeObject = 0x2002_usize as *const PyTypeObject;

    enum FakeLen {
        Value(usize),
        Failing,
    }

    struct FakeAssetBridge {
        len: FakeLen,
    }

    impl AssetBridge for FakeAssetBridge {
        fn bevy_type_id(&self) -> TypeId {
            TypeId::of::<FakeAssetType>()
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            FAKE_PTR
        }

        fn py_type<'py>(&self, _py: Python<'py>) -> Bound<'py, PyType> {
            unreachable!("not exercised by the trait-default tests")
        }

        fn name(&self) -> &'static str {
            "FakeAsset"
        }

        fn resource_id(&self, _world: &World) -> Option<ComponentId> {
            unreachable!("not exercised by the trait-default tests")
        }

        fn register_resource_id(&self, _world: &mut World) -> ComponentId {
            unreachable!("not exercised by the trait-default tests")
        }

        fn register_event_resource_id(&self, _world: &mut World) -> ComponentId {
            unreachable!("not exercised by the trait-default tests")
        }

        fn read_events(
            &self,
            _world: &World,
            _cursor: &mut Option<Box<dyn Any + Send + Sync>>,
        ) -> Vec<AssetEventRecord> {
            unreachable!("not exercised by the trait-default tests")
        }

        fn clear_events(&self, _world: &mut World) {
            unreachable!("not exercised by the trait-default tests")
        }

        fn events_is_empty(&self, _world: &World) -> bool {
            unreachable!("not exercised by the trait-default tests")
        }

        fn event_count(&self, _world: &World) -> usize {
            unreachable!("not exercised by the trait-default tests")
        }

        fn register_load_failed_resource_id(&self, _world: &mut World) -> ComponentId {
            unreachable!("not exercised by the trait-default tests")
        }

        fn read_load_failed_events(
            &self,
            _world: &World,
            _cursor: &mut Option<Box<dyn Any + Send + Sync>>,
        ) -> Vec<AssetLoadFailedRecord> {
            unreachable!("not exercised by the trait-default tests")
        }

        fn clear_load_failed_events(&self, _world: &mut World) {
            unreachable!("not exercised by the trait-default tests")
        }

        fn load_failed_events_is_empty(&self, _world: &World) -> bool {
            unreachable!("not exercised by the trait-default tests")
        }

        fn load_failed_event_count(&self, _world: &World) -> usize {
            unreachable!("not exercised by the trait-default tests")
        }

        fn get(
            &self,
            _world: &World,
            _id: UntypedAssetId,
            _validity: ValidityFlagWithMode,
            _borrow_counter: AssetBorrowCounter,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("not exercised by the trait-default tests")
        }

        fn get_mut(
            &self,
            _world: UnsafeWorldCell<'_>,
            _id: UntypedAssetId,
            _validity: ValidityFlagWithMode,
            _borrow_counter: AssetBorrowCounter,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("not exercised by the trait-default tests")
        }

        fn add(
            &self,
            _world: &mut World,
            _asset: &Bound<PyAny>,
            _py: Python,
        ) -> PyResult<UntypedHandle> {
            unreachable!("not exercised by the trait-default tests")
        }

        fn remove(&self, _world: &mut World, _id: UntypedAssetId) -> PyResult<bool> {
            unreachable!("not exercised by the trait-default tests")
        }

        fn remove_and_return(
            &self,
            _world: &mut World,
            _id: UntypedAssetId,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("not exercised by the trait-default tests")
        }

        fn len(&self, _world: &World) -> PyResult<usize> {
            match self.len {
                FakeLen::Value(count) => Ok(count),
                FakeLen::Failing => Err(PyRuntimeError::new_err("fake len failure")),
            }
        }

        fn contains(&self, _world: &World, _id: UntypedAssetId) -> PyResult<bool> {
            unreachable!("not exercised by the trait-default tests")
        }

        fn iter_pairs(
            &self,
            _world: &World,
            _validity: ValidityFlagWithMode,
            _borrow_counter: AssetBorrowCounter,
            _py: Python,
        ) -> PyResult<Vec<(UntypedAssetId, Py<PyAny>)>> {
            unreachable!("not exercised by the trait-default tests")
        }

        fn load(&self, _asset_server: &AssetServer, _path: AssetPath) -> UntypedHandle {
            unreachable!("not exercised by the trait-default tests")
        }

        fn get_handle(
            &self,
            _asset_server: &AssetServer,
            _path: AssetPath,
        ) -> Option<UntypedHandle> {
            unreachable!("not exercised by the trait-default tests")
        }

        fn clear_programmatic(&self, _world: &mut World, _verbose: bool) {
            unreachable!("not exercised by the trait-default tests")
        }
    }

    struct FakeAssetBridge2;

    impl AssetBridge for FakeAssetBridge2 {
        fn bevy_type_id(&self) -> TypeId {
            TypeId::of::<FakeAssetType2>()
        }

        fn py_type_ptr(&self) -> *const PyTypeObject {
            FAKE_PTR2
        }

        fn py_type<'py>(&self, _py: Python<'py>) -> Bound<'py, PyType> {
            unreachable!("not exercised by the registry-key tests")
        }

        fn name(&self) -> &'static str {
            "FakeAsset2"
        }

        fn resource_id(&self, _world: &World) -> Option<ComponentId> {
            unreachable!("not exercised by the registry-key tests")
        }

        fn register_resource_id(&self, _world: &mut World) -> ComponentId {
            unreachable!("not exercised by the registry-key tests")
        }

        fn register_event_resource_id(&self, _world: &mut World) -> ComponentId {
            unreachable!("not exercised by the registry-key tests")
        }

        fn read_events(
            &self,
            _world: &World,
            _cursor: &mut Option<Box<dyn Any + Send + Sync>>,
        ) -> Vec<AssetEventRecord> {
            unreachable!("not exercised by the registry-key tests")
        }

        fn clear_events(&self, _world: &mut World) {
            unreachable!("not exercised by the registry-key tests")
        }

        fn events_is_empty(&self, _world: &World) -> bool {
            unreachable!("not exercised by the registry-key tests")
        }

        fn event_count(&self, _world: &World) -> usize {
            unreachable!("not exercised by the registry-key tests")
        }

        fn register_load_failed_resource_id(&self, _world: &mut World) -> ComponentId {
            unreachable!("not exercised by the registry-key tests")
        }

        fn read_load_failed_events(
            &self,
            _world: &World,
            _cursor: &mut Option<Box<dyn Any + Send + Sync>>,
        ) -> Vec<AssetLoadFailedRecord> {
            unreachable!("not exercised by the registry-key tests")
        }

        fn clear_load_failed_events(&self, _world: &mut World) {
            unreachable!("not exercised by the registry-key tests")
        }

        fn load_failed_events_is_empty(&self, _world: &World) -> bool {
            unreachable!("not exercised by the registry-key tests")
        }

        fn load_failed_event_count(&self, _world: &World) -> usize {
            unreachable!("not exercised by the registry-key tests")
        }

        fn get(
            &self,
            _world: &World,
            _id: UntypedAssetId,
            _validity: ValidityFlagWithMode,
            _borrow_counter: AssetBorrowCounter,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("not exercised by the registry-key tests")
        }

        fn get_mut(
            &self,
            _world: UnsafeWorldCell<'_>,
            _id: UntypedAssetId,
            _validity: ValidityFlagWithMode,
            _borrow_counter: AssetBorrowCounter,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("not exercised by the registry-key tests")
        }

        fn add(
            &self,
            _world: &mut World,
            _asset: &Bound<PyAny>,
            _py: Python,
        ) -> PyResult<UntypedHandle> {
            unreachable!("not exercised by the registry-key tests")
        }

        fn remove(&self, _world: &mut World, _id: UntypedAssetId) -> PyResult<bool> {
            unreachable!("not exercised by the registry-key tests")
        }

        fn remove_and_return(
            &self,
            _world: &mut World,
            _id: UntypedAssetId,
            _py: Python,
        ) -> PyResult<Option<Py<PyAny>>> {
            unreachable!("not exercised by the registry-key tests")
        }

        fn len(&self, _world: &World) -> PyResult<usize> {
            Ok(0)
        }

        fn contains(&self, _world: &World, _id: UntypedAssetId) -> PyResult<bool> {
            unreachable!("not exercised by the registry-key tests")
        }

        fn iter_pairs(
            &self,
            _world: &World,
            _validity: ValidityFlagWithMode,
            _borrow_counter: AssetBorrowCounter,
            _py: Python,
        ) -> PyResult<Vec<(UntypedAssetId, Py<PyAny>)>> {
            unreachable!("not exercised by the registry-key tests")
        }

        fn load(&self, _asset_server: &AssetServer, _path: AssetPath) -> UntypedHandle {
            unreachable!("not exercised by the registry-key tests")
        }

        fn get_handle(
            &self,
            _asset_server: &AssetServer,
            _path: AssetPath,
        ) -> Option<UntypedHandle> {
            unreachable!("not exercised by the registry-key tests")
        }

        fn clear_programmatic(&self, _world: &mut World, _verbose: bool) {
            unreachable!("not exercised by the registry-key tests")
        }
    }

    #[test]
    fn asset_bridge_registry_lookup_by_all_three_keys() {
        let bridge1: Arc<dyn AssetBridge> = Arc::new(FakeAssetBridge {
            len: FakeLen::Value(0),
        });
        let bridge2: Arc<dyn AssetBridge> = Arc::new(FakeAssetBridge2);
        register_asset_bridge_arc(bridge1.clone());
        register_asset_bridge_arc(bridge2.clone());

        // Each key resolves to the exact registered bridge (Arc identity).
        assert!(Arc::ptr_eq(
            &bridge1,
            &get_asset_bridge_by_py_type(FAKE_PTR).expect("registered pointer 1")
        ));
        assert!(Arc::ptr_eq(
            &bridge1,
            &get_asset_bridge_by_type_id(TypeId::of::<FakeAssetType>())
                .expect("registered type id 1")
        ));
        assert!(Arc::ptr_eq(
            &bridge1,
            &get_asset_bridge_by_name("FakeAsset").expect("registered name 1")
        ));
        assert!(Arc::ptr_eq(
            &bridge2,
            &get_asset_bridge_by_py_type(FAKE_PTR2).expect("registered pointer 2")
        ));
        assert!(Arc::ptr_eq(
            &bridge2,
            &get_asset_bridge_by_type_id(TypeId::of::<FakeAssetType2>())
                .expect("registered type id 2")
        ));
        assert!(Arc::ptr_eq(
            &bridge2,
            &get_asset_bridge_by_name("FakeAsset2").expect("registered name 2")
        ));
        assert!(contains_asset_py_type(FAKE_PTR));
        assert!(contains_asset_py_type(FAKE_PTR2));

        // Unknown keys are absent; the registry holds no production entries to remove.
        let null = std::ptr::null();
        assert!(get_asset_bridge_by_py_type(null).is_none());
        assert!(!contains_asset_py_type(null));
        assert!(get_asset_bridge_by_py_type(0x9999_usize as *const PyTypeObject).is_none());
        assert!(get_asset_bridge_by_name("NotRegistered").is_none());
        assert!(get_asset_bridge_by_type_id(TypeId::of::<u8>()).is_none());
    }

    #[test]
    fn is_empty_default_derives_from_len() {
        Python::attach(|_py| {
            let world = World::new();
            let empty = FakeAssetBridge {
                len: FakeLen::Value(0),
            };
            assert!(empty.is_empty(&world).expect("is_empty"));

            let non_empty = FakeAssetBridge {
                len: FakeLen::Value(3),
            };
            assert!(!non_empty.is_empty(&world).expect("is_empty"));

            let failing = FakeAssetBridge {
                len: FakeLen::Failing,
            };
            let error = failing.is_empty(&world).unwrap_err();
            assert_eq!(format!("{error}"), "RuntimeError: fake len failure");
        });
    }

    #[test]
    fn default_try_convert_input_defers_to_the_standard_type_check() {
        Python::attach(|py| {
            let bridge = FakeAssetBridge {
                len: FakeLen::Value(0),
            };
            let input = py.eval(c"object()", None, None).unwrap();
            // `None` defers to the standard type check against the bridge's asset type.
            assert!(bridge.try_convert_input(&input, py).unwrap().is_none());
        });
    }

    #[test]
    fn event_records_carry_meaningful_discriminators() {
        // Distinct types give distinct untyped ids (type_id is part of equality).
        let id_a = Handle::<FakeAssetType>::default().id().untyped();
        let id_b = Handle::<FakeAssetType2>::default().id().untyped();
        assert_ne!(id_a, id_b);

        // Copy leaves the original usable; Eq discriminates on variant and id.
        let added_a = AssetEventRecord::Added { id: id_a };
        let added_a_copy = added_a;
        assert_ne!(added_a_copy, AssetEventRecord::Modified { id: id_a });
        assert_ne!(added_a, AssetEventRecord::Added { id: id_b });
        assert_eq!(added_a, AssetEventRecord::Added { id: id_a });

        let modified = AssetEventRecord::Modified { id: id_a };
        let removed = AssetEventRecord::Removed { id: id_a };
        let unused = AssetEventRecord::Unused { id: id_a };
        let loaded = AssetEventRecord::LoadedWithDependencies { id: id_a };
        assert_ne!(added_a, modified);
        assert_ne!(added_a, removed);
        assert_ne!(added_a, unused);
        assert_ne!(added_a, loaded);
        assert_ne!(modified, removed);
        assert_ne!(removed, unused);
        assert_ne!(unused, loaded);

        let path = AssetPath::from("fake.png");
        let first = AssetLoadFailedRecord {
            id: id_a,
            path: path.clone(),
            error: "boom".into(),
        };
        let second = AssetLoadFailedRecord {
            id: id_a,
            path: path.clone(),
            error: "boom".into(),
        };
        assert_eq!(first, second);
        let different = AssetLoadFailedRecord {
            id: id_a,
            path: path.clone(),
            error: "other".into(),
        };
        assert_ne!(first, different);
        assert_ne!(
            first,
            AssetLoadFailedRecord {
                id: id_b,
                path: path.clone(),
                error: "boom".into()
            }
        );
        assert_eq!(first.error, "boom");
        assert_eq!(first.path, path);
    }
}
