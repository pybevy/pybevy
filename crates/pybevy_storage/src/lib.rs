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
//! - `ListStorage<T>` - Generic storage for Vec<T> fields
//! - `BorrowableStorage` / `FromBorrowedStorage` - Traits for borrowed field access
//! - `AppStoreCore` - Backend-neutral App identity and ownership transitions

pub mod app_store;
pub mod asset_runtime;
pub mod batch_columns;
pub mod borrowed;
pub mod field_storage;
pub mod filtered_entity_access;
pub mod list_storage;
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
pub use asset_runtime::{AssetRuntimeCore, AssetRuntimeError};
pub use borrowed::{BorrowedMut, BorrowedRef};
pub use field_storage::{FieldStorage, FieldStorageInner};
pub use filtered_entity_access::FilteredEntityAccess;
pub use list_storage::{ListStorage, ListStorageInner, normalize_index};
pub use pyasset::{AssetBorrowCounter, AssetStorage};
pub use pycomponent::{ComponentStorage, ComponentStorageInner};
pub use pyresource::{ResourceStorage, ResourceStorageInner};
pub use storage_access::{StorageMut, StorageRef};
pub use storage_error::StorageError;
pub use storage_traits::{BorrowableStorage, FromBorrowedStorage};
pub use validity_guard::{AccessMode, ValidityFlag, ValidityFlagWithMode, ValidityGuard};
pub use value_storage::{ValueStorage, ValueStorageInner};
pub use view_bridge::{FieldOffset, FieldType, ViewBridge, ViewFieldAccess};
