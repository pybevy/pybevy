//! Runtime registries for component, asset, and resource bridges
//!
//! These registries are Bevy resources that store bridge implementations
//! registered by feature crates at plugin initialization time.

use std::{any::TypeId, collections::HashMap, sync::Arc};

use bevy::prelude::Resource;
use pyo3::ffi::PyTypeObject;

use super::{AssetBridge, ComponentBridge, ResourceBridge};

/// Runtime registry for dynamically registered component bridges.
///
/// This resource is initialized by `PyBevyCorePlugin` and feature crates
/// register their bridges during plugin initialization.
///
/// # Lookup Methods
///
/// - `get_by_py_type()` - O(1) lookup by Python type pointer
/// - `get_by_type_id()` - O(1) lookup by Rust TypeId
///
/// # Thread Safety
///
/// The registry is `Send + Sync` because:
/// - `Arc<dyn ComponentBridge>` is `Send + Sync` (trait bound)
/// - `*const PyTypeObject` pointers are stable for Python interpreter lifetime
/// - HashMap operations are safe with proper synchronization via Bevy's resource system
#[derive(Resource, Default)]
pub struct DynamicComponentRegistry {
    /// TypeId → Bridge (for Bevy-side lookups)
    by_type_id: HashMap<TypeId, Arc<dyn ComponentBridge>>,
    /// PyTypeObject* → Bridge (for Python-side lookups)
    by_py_type: HashMap<*const PyTypeObject, Arc<dyn ComponentBridge>>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter.
// The Arc<dyn ComponentBridge> is Send + Sync by trait bounds.
unsafe impl Send for DynamicComponentRegistry {}
unsafe impl Sync for DynamicComponentRegistry {}

impl DynamicComponentRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a component bridge
    ///
    /// The bridge is stored in an Arc and indexed by both TypeId and Python type pointer.
    pub fn register<B: ComponentBridge>(&mut self, bridge: B) -> &mut Self {
        self.register_arc(Arc::new(bridge))
    }

    /// Register a pre-wrapped Arc bridge (used by PyBevyPlugin for shared ownership)
    pub fn register_arc(&mut self, bridge: Arc<dyn ComponentBridge>) -> &mut Self {
        self.by_type_id
            .insert(bridge.bevy_type_id(), bridge.clone());
        self.by_py_type.insert(bridge.py_type_ptr(), bridge);
        self
    }

    /// Look up bridge by Python type object pointer
    ///
    /// Returns `None` if no bridge is registered for this type.
    pub fn get_by_py_type(&self, ptr: *const PyTypeObject) -> Option<&dyn ComponentBridge> {
        self.by_py_type.get(&ptr).map(|b| b.as_ref())
    }

    /// Look up bridge by Rust TypeId
    ///
    /// Returns `None` if no bridge is registered for this type.
    pub fn get_by_type_id(&self, type_id: TypeId) -> Option<&dyn ComponentBridge> {
        self.by_type_id.get(&type_id).map(|b| b.as_ref())
    }

    /// Check if a bridge is registered for the given Python type
    pub fn contains_py_type(&self, ptr: *const PyTypeObject) -> bool {
        self.by_py_type.contains_key(&ptr)
    }

    /// Check if a bridge is registered for the given Rust TypeId
    pub fn contains_type_id(&self, type_id: TypeId) -> bool {
        self.by_type_id.contains_key(&type_id)
    }

    /// Get the number of registered bridges
    pub fn len(&self) -> usize {
        self.by_type_id.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.by_type_id.is_empty()
    }

    /// Get all registered bridges
    pub fn all_bridges(&self) -> Vec<Arc<dyn ComponentBridge>> {
        self.by_type_id.values().cloned().collect()
    }
}

/// Runtime registry for dynamically registered asset bridges.
///
/// This resource is initialized by `PyBevyCorePlugin` and feature crates
/// register their bridges during plugin initialization.
///
/// # Lookup Methods
///
/// - `get_by_py_type()` - O(1) lookup by Python type pointer
/// - `get_by_type_id()` - O(1) lookup by Rust TypeId
#[derive(Resource, Default)]
pub struct DynamicAssetRegistry {
    /// TypeId → Bridge (for Bevy-side lookups)
    by_type_id: HashMap<TypeId, Arc<dyn AssetBridge>>,
    /// PyTypeObject* → Bridge (for Python-side lookups)
    by_py_type: HashMap<*const PyTypeObject, Arc<dyn AssetBridge>>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter.
// The Arc<dyn AssetBridge> is Send + Sync by trait bounds.
unsafe impl Send for DynamicAssetRegistry {}
unsafe impl Sync for DynamicAssetRegistry {}

impl DynamicAssetRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an asset bridge
    ///
    /// The bridge is stored in an Arc and indexed by both TypeId and Python type pointer.
    pub fn register<B: AssetBridge>(&mut self, bridge: B) -> &mut Self {
        let bridge = Arc::new(bridge);
        self.by_type_id
            .insert(bridge.bevy_type_id(), bridge.clone());
        self.by_py_type.insert(bridge.py_type_ptr(), bridge);
        self
    }

    /// Look up bridge by Python type object pointer
    ///
    /// Returns `None` if no bridge is registered for this type.
    pub fn get_by_py_type(&self, ptr: *const PyTypeObject) -> Option<&dyn AssetBridge> {
        self.by_py_type.get(&ptr).map(|b| b.as_ref())
    }

    /// Look up bridge by Rust TypeId
    ///
    /// Returns `None` if no bridge is registered for this type.
    pub fn get_by_type_id(&self, type_id: TypeId) -> Option<&dyn AssetBridge> {
        self.by_type_id.get(&type_id).map(|b| b.as_ref())
    }

    /// Check if a bridge is registered for the given Python type
    pub fn contains_py_type(&self, ptr: *const PyTypeObject) -> bool {
        self.by_py_type.contains_key(&ptr)
    }

    /// Check if a bridge is registered for the given Rust TypeId
    pub fn contains_type_id(&self, type_id: TypeId) -> bool {
        self.by_type_id.contains_key(&type_id)
    }

    /// Get the number of registered bridges
    pub fn len(&self) -> usize {
        self.by_type_id.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.by_type_id.is_empty()
    }
}

/// Generic key-value config store for plugin discovery.
///
/// Any plugin can insert JSON config under a string key during `build()`.
/// The ControlPlugin exposes these via `GET /api/v1/config/{key}`.
#[derive(Resource, Default)]
pub struct PluginConfigs {
    entries: HashMap<String, serde_json::Value>,
}

impl PluginConfigs {
    pub fn insert(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.entries.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.entries.get(key)
    }

    pub fn all(&self) -> &HashMap<String, serde_json::Value> {
        &self.entries
    }
}

/// Runtime registry for dynamically registered resource bridges.
///
/// This registry stores bridge implementations for resources
/// registered by feature crates at plugin initialization time.
///
/// # Lookup Methods
///
/// - `get_by_py_type()` - O(1) lookup by Python type pointer
/// - `get_by_type_id()` - O(1) lookup by Rust TypeId
#[derive(Resource, Default)]
pub struct DynamicResourceRegistry {
    /// TypeId → Bridge (for Bevy-side lookups)
    by_type_id: HashMap<TypeId, Arc<dyn ResourceBridge>>,
    /// PyTypeObject* → Bridge (for Python-side lookups)
    by_py_type: HashMap<*const PyTypeObject, Arc<dyn ResourceBridge>>,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter.
// The Arc<dyn ResourceBridge> is Send + Sync by trait bounds.
unsafe impl Send for DynamicResourceRegistry {}
unsafe impl Sync for DynamicResourceRegistry {}

impl DynamicResourceRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a resource bridge
    ///
    /// The bridge is stored in an Arc and indexed by both TypeId and Python type pointer.
    pub fn register<B: ResourceBridge>(&mut self, bridge: B) -> &mut Self {
        let bridge = Arc::new(bridge);
        self.by_type_id
            .insert(bridge.bevy_type_id(), bridge.clone());
        self.by_py_type.insert(bridge.py_type_ptr(), bridge);
        self
    }

    /// Look up bridge by Python type object pointer
    ///
    /// Returns `None` if no bridge is registered for this type.
    pub fn get_by_py_type(&self, ptr: *const PyTypeObject) -> Option<&dyn ResourceBridge> {
        self.by_py_type.get(&ptr).map(|b| b.as_ref())
    }

    /// Look up bridge by Rust TypeId
    ///
    /// Returns `None` if no bridge is registered for this type.
    pub fn get_by_type_id(&self, type_id: TypeId) -> Option<&dyn ResourceBridge> {
        self.by_type_id.get(&type_id).map(|b| b.as_ref())
    }

    /// Check if a bridge is registered for the given Python type
    pub fn contains_py_type(&self, ptr: *const PyTypeObject) -> bool {
        self.by_py_type.contains_key(&ptr)
    }

    /// Check if a bridge is registered for the given Rust TypeId
    pub fn contains_type_id(&self, type_id: TypeId) -> bool {
        self.by_type_id.contains_key(&type_id)
    }

    /// Get the number of registered bridges
    pub fn len(&self) -> usize {
        self.by_type_id.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.by_type_id.is_empty()
    }
}
