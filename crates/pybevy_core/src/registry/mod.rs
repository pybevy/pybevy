//! Runtime type registries for PyBevy
//!
//! This module provides the bridge traits and global registry that enable
//! feature crates to register their types without the core crate needing
//! to import them at compile time.

mod asset_bridge;
mod asset_input_converter;
mod batch_bridge;
pub mod batchable_field;
mod component_bridge;
pub mod global_registry;
mod message_bridge;
mod registries;
mod resource_bridge;
pub mod rust_batch;

pub use asset_bridge::{AssetBridge, AssetEventRecord, AssetLoadFailedRecord};
pub use asset_input_converter::AssetInputConverter;
pub use batch_bridge::{
    BatchComponent, PreparedBatchComponent, PreparedNativeBatch, PreparedNativeUniform,
    PreparedNativeUniformWith, PreparedUniformComponent, PreparedUniformFn,
};
pub use batchable_field::{
    BatchFieldMeta, BatchableField, batch_field_meta_for, batch_integer_column_of,
    check_batch_field, field_type_of, set_field_from_numpy,
};
pub use component_bridge::{ComponentBridge, ExtractFn};
pub use global_registry::{ComponentBatchInsertFn, ComponentBatchMeta, ComponentBatchPrepareFn};
pub use message_bridge::{
    MessageBridge, ReaderCursorOp, apply_cursor_op, apply_reader_cursor, register_native_message,
};
pub use registries::PluginConfigs;
pub use resource_bridge::ResourceBridge;
pub use rust_batch::{
    PyRustComponentBatch, check_f32_integer_batch_column, check_integer_batch_column,
    portable_f32_batch_column,
};
