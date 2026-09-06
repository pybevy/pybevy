//! Resource bridge trait for runtime type dispatch
//!
//! This module provides the `ResourceBridge` trait that allows feature crates
//! to register their Bevy resources without the core crate needing to import them.
//!
//! 1. Feature crate implements `ResourceBridge` for each resource
//! 2. Feature crate registers bridges at startup
//! 3. Core uses bridges via runtime dispatch (no compile-time coupling)

use std::any::TypeId;

use bevy::ecs::{
    component::ComponentId,
    entity::Entity,
    world::{EntityRef, World, unsafe_world_cell::UnsafeWorldCell},
};
use pyo3::{ffi::PyTypeObject, prelude::*, types::PyType};

use crate::{FilteredEntityAccess, ValidityFlagWithMode};

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

    fn is_mutable(&self) -> bool;

    /// Whether Full hot reload must preserve this resource verbatim.
    ///
    /// Use this for engine-owned registries whose `Default` value is not
    /// equivalent to the state assembled by plugin initialization. Replacing
    /// such a resource would invalidate the native systems that remain in the
    /// app across Python scene reloads.
    fn preserve_on_reload(&self) -> bool;

    fn extract(
        &self,
        entity: &mut FilteredEntityAccess,
        component_id: ComponentId,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Py<PyAny>>;

    fn entity_contains(&self, entity: &EntityRef) -> bool;

    /// # Safety
    /// `world_ptr` and `validity` must satisfy the returned wrapper's shared-access lifetime.
    unsafe fn extract_from_entity_ref(
        &self,
        entity_id: Entity,
        world_ptr: *mut World,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>>;

    /// # Safety
    /// `world_ptr` and `validity` must satisfy the returned wrapper's mutable-access lifetime.
    unsafe fn extract_from_entity_mut(
        &self,
        entity_id: Entity,
        world_ptr: *mut World,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>>;

    /// Get resource from world (read-only access)
    fn get(&self, world: &World, validity: ValidityFlagWithMode, py: Python)
    -> PyResult<Py<PyAny>>;

    /// Get resource from world (mutable access)
    fn get_mut(
        &self,
        world: &mut World,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Py<PyAny>>;

    /// Get resource through an [`UnsafeWorldCell`] (read-only), touching only this
    /// bridge's `Resource` type instead of borrowing the whole `World`.
    ///
    /// Used by `DynamicSystem::run_unsafe`, which holds a non-exclusive
    /// `UnsafeWorldCell` and must not conjure `&World`/`&mut World`.
    ///
    /// # Safety
    /// The caller must guarantee that `DynamicSystem::initialize` declared read
    /// access to this resource and that Bevy's executor prevents any concurrent
    /// writer, so the cell's unchecked resource read is unique.
    unsafe fn get_from_cell(
        &self,
        cell: UnsafeWorldCell,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Py<PyAny>>;

    /// Get resource through an [`UnsafeWorldCell`] (mutable), touching only this
    /// bridge's `Resource` type. Read-only bridges (`no_mut`) return read access.
    ///
    /// # Safety
    /// The caller must guarantee that `DynamicSystem::initialize` declared write
    /// access to this resource and that Bevy's executor prevents any concurrent
    /// access, so the cell's unchecked resource borrow is unique.
    unsafe fn get_mut_from_cell(
        &self,
        cell: UnsafeWorldCell,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Py<PyAny>>;

    /// Insert resource into world
    fn insert(&self, world: &mut World, resource: &Bound<PyAny>) -> PyResult<()>;

    /// Remove resource from world
    fn remove(&self, world: &mut World);

    /// Check if resource exists in world
    fn contains_in_world(&self, world: &World) -> bool;

    /// Get the ComponentId for this resource type in the world
    ///
    /// Returns None if the resource hasn't been registered yet.
    fn resource_id(&self, world: &World) -> Option<ComponentId>;

    /// Register (get-or-create) the ComponentId for this resource type.
    ///
    /// Unlike `resource_id`, this never returns None: it creates the id when the
    /// resource is absent so `DynamicSystem::initialize` can declare access even
    /// when the resource is inserted later (e.g. from a startup Commands). The
    /// created id is TypeId-keyed and equals the one a later insertion resolves to.
    fn register_resource_id(&self, world: &mut World) -> ComponentId;

    /// Reset resource to its default value (T::default()).
    ///
    /// Returns `true` if the resource was reset, `false` if the type has no Default impl
    /// (declared with `no_default` flag). Used during hot reload to restore Bevy-plugin
    /// resources to their initial state unless [`Self::preserve_on_reload`]
    /// opts the resource out of replacement.
    fn reset_to_default(&self, world: &mut World) -> bool;
}
