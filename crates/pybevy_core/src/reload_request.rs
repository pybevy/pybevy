//! Cross-crate reload request mailbox and shared resources.
//!
//! This module provides simple Bevy Resources that allow `pybevy_mcp`
//! to communicate with the main `pybevy` crate without direct dependencies.
//!
//! Flow:
//! 1. `pybevy_mcp` writes a `ReloadRequestMode` into `PendingReloadRequest`
//! 2. The hot reload system in the main crate checks and drains it each frame

use std::collections::HashMap;

use bevy::{ecs::component::ComponentId, prelude::Resource};
use pyo3::{Py, PyAny, ffi::PyTypeObject};

/// The mode of reload to perform
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadRequestMode {
    Full,
    Partial,
}

/// Mailbox resource: MCP writes, hot reload reads.
#[derive(Resource, Default)]
pub struct PendingReloadRequest {
    pub mode: Option<ReloadRequestMode>,
}

/// Stores the last Python system error for MCP to read.
/// Written by DynamicSystem on error, read by MCP's `get_last_error`.
#[derive(Resource, Default, Clone)]
pub struct LastSystemError {
    pub error: Option<String>,
    /// Full Python traceback with file paths and line numbers.
    pub traceback: Option<String>,
    pub timestamp_secs: f64,
}

/// Metadata for a registered custom Python component.
#[derive(Debug, Clone)]
pub struct CustomComponentEntry {
    /// Python type pointer (stable for interpreter lifetime)
    pub type_ptr: *const PyTypeObject,
    /// Python class name (e.g., "Player", "Health")
    pub name: String,
    /// Whether this component uses PyObject storage (true) or Wrapper storage (false)
    pub is_pyobject_storage: bool,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for CustomComponentEntry {}
unsafe impl Sync for CustomComponentEntry {}

/// Registry of custom Python components, accessible from MCP handlers.
///
/// Written by `register_custom_component()` in the main crate,
/// read by MCP handlers to identify custom component names and extract fields.
#[derive(Resource, Default)]
pub struct CustomComponentInfo {
    entries: HashMap<ComponentId, CustomComponentEntry>,
}

impl CustomComponentInfo {
    /// Register a custom component entry
    pub fn insert(&mut self, id: ComponentId, entry: CustomComponentEntry) {
        self.entries.insert(id, entry);
    }

    /// Look up a custom component by ComponentId
    pub fn get(&self, id: ComponentId) -> Option<&CustomComponentEntry> {
        self.entries.get(&id)
    }

    /// Iterate over all registered custom components
    pub fn iter(&self) -> impl Iterator<Item = (ComponentId, &CustomComponentEntry)> {
        self.entries.iter().map(|(&id, entry)| (id, entry))
    }

    /// Clear all entries (used during full reload)
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Update the type_ptr for an existing entry (used during hot reload aliasing).
    /// After reload, Python classes get new PyTypeObject pointers; this keeps the
    /// entry pointing at the current (live) type object.
    pub fn update_type_ptr(
        &mut self,
        component_id: ComponentId,
        new_type_ptr: *const PyTypeObject,
    ) {
        if let Some(entry) = self.entries.get_mut(&component_id) {
            entry.type_ptr = new_type_ptr;
        }
    }
}

/// Metadata for a registered custom Python resource.
#[derive(Debug, Clone)]
pub struct CustomResourceEntry {
    /// Python type pointer (stable for interpreter lifetime)
    pub type_ptr: *const PyTypeObject,
    /// Python class name (e.g., "GameState", "Score")
    pub name: String,
}

// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter
unsafe impl Send for CustomResourceEntry {}
unsafe impl Sync for CustomResourceEntry {}

/// Registry of custom Python resources, accessible from MCP handlers.
///
/// Written by `register_custom_resource()` in the main crate,
/// read by MCP handlers to include custom resources in `list_resources`.
#[derive(Resource, Default)]
pub struct CustomResourceInfo {
    entries: HashMap<ComponentId, CustomResourceEntry>,
}

impl CustomResourceInfo {
    /// Register a custom resource entry
    pub fn insert(&mut self, id: ComponentId, entry: CustomResourceEntry) {
        self.entries.insert(id, entry);
    }

    /// Look up a custom resource by ComponentId
    pub fn get(&self, id: ComponentId) -> Option<&CustomResourceEntry> {
        self.entries.get(&id)
    }

    /// Iterate over all registered custom resources
    pub fn iter(&self) -> impl Iterator<Item = (ComponentId, &CustomResourceEntry)> {
        self.entries.iter().map(|(&id, entry)| (id, entry))
    }

    /// Clear all entries (used during full reload)
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Update the type_ptr for an existing entry (used during hot reload aliasing).
    pub fn update_type_ptr(
        &mut self,
        component_id: ComponentId,
        new_type_ptr: *const PyTypeObject,
    ) {
        if let Some(entry) = self.entries.get_mut(&component_id) {
            entry.type_ptr = new_type_ptr;
        }
    }
}

/// Result of a reload operation, readable by MCP.
///
/// Written by `perform_reload()` in the main crate after each reload attempt.
#[derive(Resource, Default, Clone)]
pub struct ReloadResult {
    /// Whether the reload was escalated from Partial to Full
    pub escalated: bool,
    /// Reason for escalation, if any
    pub escalation_reason: Option<String>,
    /// The mode that was actually used
    pub actual_mode: Option<ReloadRequestMode>,
    /// Whether the last reload attempt failed
    pub failed: bool,
    /// Reason for failure, if any
    pub failure_reason: Option<String>,
    /// Whether the app is running code from a previous generation after a failure
    pub running_previous_generation: bool,
    /// Plugin names added since last reload (restart may be required)
    pub plugins_added: Option<Vec<String>>,
    /// Plugin names removed since last reload (restart required to take effect)
    pub plugins_removed: Option<Vec<String>>,
    /// System names removed or renamed since last reload (load_scene required to clear stale schedule entries)
    pub systems_removed: Option<Vec<String>>,
}

/// Storage for custom Python resources.
/// Maps ComponentIds to Python objects. Lives in pybevy_core so that
/// both the main crate and pybevy_control can access it.
#[derive(Default, Resource)]
pub struct PyResourceStorage {
    pub resources: HashMap<ComponentId, Py<PyAny>>,
}

// SAFETY: We ensure all Python access happens within Python::attach
unsafe impl Send for PyResourceStorage {}
unsafe impl Sync for PyResourceStorage {}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_component_id(index: usize) -> ComponentId {
        ComponentId::new(index)
    }

    fn make_entry(name: &str) -> CustomComponentEntry {
        CustomComponentEntry {
            type_ptr: std::ptr::null(),
            name: name.to_string(),
            is_pyobject_storage: false,
        }
    }

    fn make_resource_entry(name: &str) -> CustomResourceEntry {
        CustomResourceEntry {
            type_ptr: std::ptr::null(),
            name: name.to_string(),
        }
    }

    #[test]
    fn reload_mode_equality() {
        assert_eq!(ReloadRequestMode::Full, ReloadRequestMode::Full);
        assert_eq!(ReloadRequestMode::Partial, ReloadRequestMode::Partial);
        assert_ne!(ReloadRequestMode::Full, ReloadRequestMode::Partial);
    }

    #[test]
    fn pending_reload_default_is_none() {
        let req = PendingReloadRequest::default();
        assert!(req.mode.is_none());
    }

    #[test]
    fn component_info_insert_and_get() {
        let mut info = CustomComponentInfo::default();
        let id = make_component_id(0);
        info.insert(id, make_entry("Health"));
        assert!(info.get(id).is_some());
        assert_eq!(info.get(id).unwrap().name, "Health");
    }

    #[test]
    fn component_info_get_missing() {
        let info = CustomComponentInfo::default();
        assert!(info.get(make_component_id(99)).is_none());
    }

    #[test]
    fn component_info_overwrite() {
        let mut info = CustomComponentInfo::default();
        let id = make_component_id(0);
        info.insert(id, make_entry("Old"));
        info.insert(id, make_entry("New"));
        assert_eq!(info.get(id).unwrap().name, "New");
    }

    #[test]
    fn component_info_iter() {
        let mut info = CustomComponentInfo::default();
        info.insert(make_component_id(0), make_entry("A"));
        info.insert(make_component_id(1), make_entry("B"));
        let names: Vec<String> = info.iter().map(|(_, e)| e.name.clone()).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"A".to_string()));
        assert!(names.contains(&"B".to_string()));
    }

    #[test]
    fn component_info_clear() {
        let mut info = CustomComponentInfo::default();
        info.insert(make_component_id(0), make_entry("A"));
        info.insert(make_component_id(1), make_entry("B"));
        info.clear();
        assert!(info.get(make_component_id(0)).is_none());
        assert_eq!(info.iter().count(), 0);
    }

    #[test]
    fn component_info_update_type_ptr() {
        let mut info = CustomComponentInfo::default();
        let id = make_component_id(0);
        info.insert(id, make_entry("Health"));
        assert!(info.get(id).unwrap().type_ptr.is_null());

        let fake_ptr = 0xDEAD as *const PyTypeObject;
        info.update_type_ptr(id, fake_ptr);
        assert_eq!(info.get(id).unwrap().type_ptr, fake_ptr);
    }

    #[test]
    fn component_info_update_type_ptr_missing_is_noop() {
        let mut info = CustomComponentInfo::default();
        let fake_ptr = 0xDEAD as *const PyTypeObject;
        // Should not panic
        info.update_type_ptr(make_component_id(99), fake_ptr);
    }

    #[test]
    fn resource_info_insert_and_get() {
        let mut info = CustomResourceInfo::default();
        let id = make_component_id(0);
        info.insert(id, make_resource_entry("Score"));
        assert_eq!(info.get(id).unwrap().name, "Score");
    }

    #[test]
    fn resource_info_clear() {
        let mut info = CustomResourceInfo::default();
        info.insert(make_component_id(0), make_resource_entry("A"));
        info.clear();
        assert_eq!(info.iter().count(), 0);
    }

    #[test]
    fn resource_info_update_type_ptr() {
        let mut info = CustomResourceInfo::default();
        let id = make_component_id(0);
        info.insert(id, make_resource_entry("Score"));

        let fake_ptr = 0xBEEF as *const PyTypeObject;
        info.update_type_ptr(id, fake_ptr);
        assert_eq!(info.get(id).unwrap().type_ptr, fake_ptr);
    }

    #[test]
    fn reload_result_default() {
        let r = ReloadResult::default();
        assert!(!r.escalated);
        assert!(!r.failed);
        assert!(r.actual_mode.is_none());
        assert!(r.escalation_reason.is_none());
        assert!(r.failure_reason.is_none());
        assert!(!r.running_previous_generation);
        assert!(r.plugins_added.is_none());
        assert!(r.plugins_removed.is_none());
        assert!(r.systems_removed.is_none());
    }

    #[test]
    fn last_system_error_default() {
        let e = LastSystemError::default();
        assert!(e.error.is_none());
        assert!(e.traceback.is_none());
        assert_eq!(e.timestamp_secs, 0.0);
    }
}
