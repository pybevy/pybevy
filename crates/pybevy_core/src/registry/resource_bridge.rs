//! Resource bridge trait for runtime type dispatch
//!
//! This module provides the `ResourceBridge` trait that allows feature crates
//! to register their Bevy resources without the core crate needing to import them.
//!
//! 1. Feature crate implements `ResourceBridge` for each resource
//! 2. Feature crate registers bridges at startup
//! 3. Core uses bridges via runtime dispatch (no compile-time coupling)

use std::any::TypeId;

use bevy::ecs::{component::ComponentId, world::World};
use pyo3::{ffi::PyTypeObject, prelude::*, types::PyType};

use crate::ValidityFlagWithMode;

/// Trait that bridges a Bevy resource to its Python wrapper.
///
/// Each feature crate implements this for its resources. The trait provides
/// all the methods needed for:
/// - Type identification (Rust TypeId, Python type object)
/// - Getting resources from World (for Res/ResMut)
/// - Inserting resources into World
/// - Removing resources from World
///
/// # Safety
///
/// The `get` and `get_mut` methods use raw pointers internally. Implementations must ensure:
/// - The validity flag is checked before dereferencing
/// - The borrowed reference doesn't outlive the system execution
pub trait ResourceBridge: Send + Sync + 'static {
    /// Rust TypeId of the Bevy resource
    fn bevy_type_id(&self) -> TypeId;

    /// Python type object pointer for type matching
    ///
    /// Used for O(1) lookup in HashMap when dispatching from Python types.
    fn py_type_ptr(&self) -> *const PyTypeObject;

    /// Get Python type object
    fn py_type<'py>(&self, py: Python<'py>) -> Bound<'py, PyType>;

    /// Human-readable name for error messages
    fn name(&self) -> &'static str;

    /// Get resource from world (read-only access)
    ///
    /// # Arguments
    ///
    /// * `world` - World reference for accessing the resource
    /// * `validity` - Validity flag for borrowed reference tracking
    /// * `py` - Python GIL token
    ///
    /// # Errors
    ///
    /// Returns error if resource is not found or Python conversion fails.
    fn get(&self, world: &World, validity: ValidityFlagWithMode, py: Python)
    -> PyResult<Py<PyAny>>;

    /// Get resource from world (mutable access)
    ///
    /// # Arguments
    ///
    /// * `world` - Mutable world reference for accessing the resource
    /// * `validity` - Validity flag for borrowed reference tracking
    /// * `py` - Python GIL token
    ///
    /// # Errors
    ///
    /// Returns error if resource is not found or Python conversion fails.
    fn get_mut(
        &self,
        world: &mut World,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Py<PyAny>>;

    /// Insert resource into world
    ///
    /// # Arguments
    ///
    /// * `world` - Mutable world reference
    /// * `resource` - Python object to convert and insert
    ///
    /// # Errors
    ///
    /// Returns error if Python conversion or insertion fails.
    fn insert(&self, world: &mut World, resource: &Bound<PyAny>) -> PyResult<()>;

    /// Remove resource from world
    ///
    /// # Arguments
    ///
    /// * `world` - Mutable world reference
    fn remove(&self, world: &mut World);

    /// Check if resource exists in world
    ///
    /// # Arguments
    ///
    /// * `world` - World reference
    ///
    /// # Returns
    ///
    /// True if the resource exists in the world
    fn contains_in_world(&self, world: &World) -> bool;

    /// Get the ComponentId for this resource type in the world
    ///
    /// Returns None if the resource hasn't been registered yet.
    fn resource_id(&self, world: &World) -> Option<ComponentId>;

    /// Reset resource to its default value (T::default()).
    ///
    /// Returns `true` if the resource was reset, `false` if the type has no Default impl
    /// (declared with `no_default` flag). Used during hot reload to restore Bevy-plugin
    /// resources to their initial state.
    fn reset_to_default(&self, world: &mut World) -> bool;
}
