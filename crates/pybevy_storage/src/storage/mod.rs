//! Storage types for PyBevy wrappers
//!
//! This module provides generic storage mechanisms for components, resources, and assets
//! that support both owned (Python-created) and borrowed (Bevy-stored) instances.

pub mod asset_storage;

pub use asset_storage::AssetStorage;
