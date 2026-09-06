//! Storage primitives for PyBevy
//!
//! ## Storage Primitives
//!
//! - `ValidityFlag` / `ValidityFlagWithMode` - Runtime validity tracking
//! - `ValidityGuard` - RAII guard for system execution scope
//! - `ValueStorage<T>` - Generic storage for Copy types (Vec3, Quat, etc.)
//! - `FieldStorage<T>` - Generic storage for non-Copy types (TextureAtlas, etc.)
//! - `ComponentStorage<T>` - Generic storage for ECS components
//! - `ResourceStorage<T>` - Generic storage for ECS resources
//! - `AssetStorage<T>` - Generic storage for Bevy assets
//! - `BorrowableStorage` / `FromBorrowedStorage` - Traits for borrowed field access
//! - `AppStoreCore` - Backend-neutral App identity and ownership transitions

pub mod app_store;
pub mod asset_access_registry;
pub mod asset_path;
pub mod asset_runtime;
pub mod batch_columns;
pub mod borrowed;
pub mod component_change;
pub mod field_storage;
pub mod filtered_entity_access;
pub mod logical_type;
pub mod plugin_group;
pub mod pyasset;
pub mod pycomponent;
pub mod pyresource;
pub mod storage_access;
pub mod storage_error;
pub mod storage_traits;
pub mod validity_guard;
pub mod value_storage;
pub mod view_bridge;

pub use app_store::{
    AllocatedAppId, AppId, AppLifecycle, AppOperation, AppRestoreError, AppStoreCore,
    AppStoreError, BorrowedApps, DrainOutcome, DrainedApps, allocate_id, consume_unstored_id,
};
pub use asset_access_registry::{
    ActiveAssetAccess, AssetAccessRegistry, AssetAccessScope, AssetResourceReadGuard,
    AssetResourceState, AssetResourceWriteGuard, PendingViewClaim, ReadViewClaim, ViewCounters,
    ensure_asset_access_registry,
};
pub(crate) use asset_path::{
    AssetPath, ErasedResolvedMut, ErasedResolvedRef, ErasedRevalidatingSource,
};
pub use asset_path::{
    ReadField, ReadIndex, ReadKey, ReadVariant, RevalidatingMut, RevalidatingRef,
    RevalidatingSource, WriteField, WriteIndex, WriteKey, WriteVariant,
};
pub use asset_runtime::{AssetRuntimeCore, AssetRuntimeError};
pub use borrowed::{BorrowedMut, BorrowedRef};
pub use component_change::ComponentWriteContext;
pub use field_storage::{FieldStorage, FieldStorageInner};
pub use filtered_entity_access::FilteredEntityAccess;
pub use logical_type::{LogicalTypeId, LogicalTypeMap};
pub use plugin_group::{DefaultPluginKind, PluginGroupAddition, PluginGroupPlacement};
pub use pyasset::{AssetBorrowCounter, AssetStorage};
pub use pycomponent::{ComponentStorage, ComponentStorageInner};
pub use pyresource::{ResourceStorage, ResourceStorageInner};
pub use storage_access::{StorageMut, StorageRef};
pub use storage_error::StorageError;
pub use storage_traits::{BorrowableStorage, FromBorrowedStorage, computed_owned};
pub use validity_guard::{AccessMode, ValidityFlag, ValidityFlagWithMode, ValidityGuard};
pub use value_storage::{ValueStorage, ValueStorageInner};
pub use view_bridge::{FieldOffset, FieldType, ViewBridge};

#[cfg(all(test, target_pointer_width = "64"))]
mod layout_tests {
    use std::mem::size_of;

    use bevy::{
        asset::Asset,
        ecs::{component::Component, resource::Resource},
        reflect::TypePath,
    };

    use super::{
        AssetRuntimeCore, AssetStorage, BorrowedMut, ComponentStorage, FieldStorage,
        ResourceStorage, ValidityFlagWithMode, ValueStorage,
    };

    #[derive(Asset, TypePath)]
    struct TestAsset;

    #[derive(Component)]
    struct TestComponent;

    #[derive(Resource)]
    struct TestResource;

    #[test]
    fn storage_layouts_are_intentional() {
        assert_eq!(size_of::<BorrowedMut<u8>>(), 64);
        assert_eq!(size_of::<ValidityFlagWithMode>(), 64);
        assert_eq!(size_of::<AssetRuntimeCore<u8>>(), 104);
        assert_eq!(size_of::<ComponentStorage<TestComponent>>(), 64);
        assert_eq!(size_of::<ValueStorage<f32>>(), 64);
        assert_eq!(size_of::<FieldStorage<String>>(), 64);
        assert_eq!(size_of::<ResourceStorage<TestResource>>(), 64);
        assert_eq!(size_of::<AssetStorage<TestAsset>>(), 144);
    }
}
