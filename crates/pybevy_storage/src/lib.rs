//! PyO3-free storage primitives for PyBevy
//!
//! This crate provides the foundational storage types that are independent of PyO3,
//! enabling reuse from both the CPython (PyO3) and RustPython/WASM backends.
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

pub mod component_storage;
pub mod field_storage;
pub mod list_storage;
pub mod resource_storage;
pub mod storage;
pub mod storage_error;
pub mod storage_traits;
pub mod validity_guard;
pub mod value_storage;
pub mod view_bridge;

pub use component_storage::{ComponentStorage, ComponentStorageInner};
pub use field_storage::{FieldStorage, FieldStorageInner};
pub use list_storage::{ListStorage, ListStorageInner, normalize_index};
pub use resource_storage::{ResourceStorage, ResourceStorageInner};
pub use storage::AssetStorage;
pub use storage_error::StorageError;
pub use storage_traits::{BorrowableStorage, FromBorrowedStorage};
pub use validity_guard::{AccessMode, ValidityFlag, ValidityFlagWithMode, ValidityGuard};
pub use value_storage::{ValueStorage, ValueStorageInner};
pub use view_bridge::{FieldOffset, ViewBridge, ViewFieldAccess};
