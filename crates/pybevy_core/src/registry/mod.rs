//! Runtime type registries for PyBevy
//!
//! This module provides the bridge traits and registries that enable
//! feature crates to register their types without the core crate needing
//! to import them at compile time.
//!
//! # Architecture
//!
//! ```text
//! pybevy_core (this crate)
//!   └── defines: ComponentBridge, AssetBridge, ResourceBridge traits
//!   └── defines: DynamicComponentRegistry, DynamicAssetRegistry, DynamicResourceRegistry resources
//!
//! pybevy_audio, pybevy_light, etc. (feature crates)
//!   └── implements: ComponentBridge for each component
//!   └── implements: AssetBridge for each asset
//!   └── implements: ResourceBridge for each resource
//!   └── registers: bridges during plugin initialization
//!
//! pybevy (main crate)
//!   └── uses: registries for runtime dispatch
//!   └── falls back: to static PyComponentType/PyResourceType for built-in types
//! ```
//!
//! # Usage
//!
//! Feature crates implement the bridge traits:
//!
//! ```ignore
//! // In pybevy_audio/src/lib.rs
//! pub struct GlobalVolumeBridge;
//!
//! impl ResourceBridge for GlobalVolumeBridge {
//!     fn bevy_type_id(&self) -> TypeId { TypeId::of::<GlobalVolume>() }
//!     fn py_type_ptr(&self) -> *const PyTypeObject { /* ... */ }
//!     // ... other methods
//! }
//! ```
//!
//! Feature crates register bridges:
//!
//! ```ignore
//! // In pybevy_audio/src/lib.rs
//! pub fn register_audio_bridges() {
//!     global_registry::register_component_bridge(AudioPlayerBridge);
//!     global_registry::register_resource_bridge(GlobalVolumeBridge);
//! }
//! ```

mod asset_bridge;
mod batch_bridge;
pub mod batchable_field;
mod component_bridge;
pub mod global_registry;
mod message_bridge;
mod registries;
mod resource_bridge;
pub mod rust_batch;

pub use asset_bridge::AssetBridge;
pub use batch_bridge::BatchComponent;
pub use batchable_field::{
    BatchFieldMeta, BatchableField, batch_field_meta_for, field_offset_view_meta_for,
    set_field_from_numpy,
};
pub use component_bridge::{ComponentBridge, ExtractFn};
pub use global_registry::{ComponentBatchInsertFn, ComponentBatchMeta};
pub use message_bridge::MessageBridge;
pub use registries::{
    DynamicAssetRegistry, DynamicComponentRegistry, DynamicResourceRegistry, PluginConfigs,
};
pub use resource_bridge::ResourceBridge;
pub use rust_batch::PyRustComponentBatch;
