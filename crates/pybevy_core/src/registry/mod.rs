//! Runtime type registries for PyBevy
//!
//! This module provides the bridge traits and global registry that enable
//! feature crates to register their types without the core crate needing
//! to import them at compile time.

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
pub use registries::PluginConfigs;
pub use resource_bridge::ResourceBridge;
pub use rust_batch::PyRustComponentBatch;
