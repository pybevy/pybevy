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
