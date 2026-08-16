//! Component bridge trait for runtime type dispatch
//!
//! This module provides the `ComponentBridge` trait that allows feature crates
//! to register their Bevy components without the core crate needing to import them.
//!
//! 1. Feature crate implements `ComponentBridge` for each component
//! 2. Feature crate registers bridges via `global_registry` at init time
//! 3. Core uses bridges via runtime dispatch (no compile-time coupling)

use std::any::TypeId;

use bevy::ecs::{
    component::ComponentId,
    entity::Entity,
    world::{EntityRef, EntityWorldMut, World},
};
use pyo3::{ffi::PyTypeObject, prelude::*, types::PyType};

use crate::{FilteredEntityAccess, PreparedUniformComponent, ValidityFlagWithMode, ViewBridge};

/// Function pointer type for component extraction.
///
/// This allows caching the extract function directly to avoid vtable dispatch.
/// Takes a `FilteredEntityAccess` which wraps either `FilteredEntityRef` (read-only)
/// or `FilteredEntityMut` (read-write) depending on query mutability.
pub type ExtractFn =
    fn(&mut FilteredEntityAccess, ComponentId, ValidityFlagWithMode, Python) -> PyResult<Py<PyAny>>;

/// Trait that bridges a Bevy component to its Python wrapper.
///
/// Each feature crate implements this for its components. The trait provides
/// all the methods needed for:
/// - Type identification (Rust TypeId, Python type object)
/// - Registration with Bevy world
/// - Extraction from entities (for Query)
/// - Insertion into entities (for Commands.spawn/insert)
/// - Containment checks
///
/// # Safety
///
/// The `extract` method uses raw pointers internally. Implementations must ensure:
/// - The validity flag is checked before dereferencing
/// - The borrowed reference doesn't outlive the system execution
pub trait ComponentBridge: Send + Sync + 'static {
    /// Rust TypeId of the Bevy component
    fn bevy_type_id(&self) -> TypeId;

    /// Python type object pointer for type matching
    ///
    /// Used for O(1) lookup in HashMap when dispatching from Python types.
    fn py_type_ptr(&self) -> *const PyTypeObject;

    /// Get Python type object
    fn py_type<'py>(&self, py: Python<'py>) -> Bound<'py, PyType>;

    /// Human-readable name for error messages
    fn name(&self) -> &'static str;

    /// Whether Python-facing APIs may insert this component.
    fn can_insert(&self) -> bool;

    /// For a Bevy `Relationship` component, the Python field holding the
    /// related entity.
    ///
    /// Two consequences for callers that build components from JSON: an
    /// integer on that field is an entity id and must become an `Entity`, and
    /// a write must rebuild and re-insert the whole component rather than
    /// setting the field in place. Bevy's relationship hooks only run on
    /// insert/replace, so an in-place write desynchronizes the
    /// `RelationshipTarget` (see `Relationship::set_risky`).
    fn relationship_field(&self) -> Option<&'static str> {
        None
    }

    /// Register component with Bevy world and return its ComponentId
    fn register(&self, world: &mut World) -> ComponentId;

    /// Extract component from entity and return as Python object
    ///
    /// # Arguments
    ///
    /// * `entity` - Mutable filtered entity reference from query iteration
    /// * `component_id` - Pre-registered ComponentId for this component
    /// * `validity` - Validity flag for borrowed reference tracking
    /// * `py` - Python GIL token
    ///
    /// # Errors
    ///
    /// Returns error if component extraction or Python conversion fails.
    fn extract(
        &self,
        entity: &mut FilteredEntityAccess,
        component_id: ComponentId,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Py<PyAny>>;

    /// Get a function pointer for component extraction.
    ///
    /// This allows caching the extract function directly to avoid vtable dispatch
    /// during per-entity iteration. The returned function pointer can be called
    /// directly without going through the trait object.
    fn extract_fn(&self) -> ExtractFn;

    /// Insert component into entity via world access
    ///
    /// # Arguments
    ///
    /// * `world` - Mutable world reference
    /// * `entity` - Entity to insert component into
    /// * `component` - Python object to convert and insert
    ///
    /// # Errors
    ///
    /// Returns error if Python conversion or insertion fails.
    fn insert(&self, world: &mut World, entity: Entity, component: &Bound<PyAny>) -> PyResult<()>;

    /// Insert component directly into an EntityWorldMut
    ///
    /// This is used by batch spawn to avoid double-mutable-borrow issues.
    /// Default implementation panics - bridges should override this.
    ///
    /// # Arguments
    ///
    /// * `entity` - Mutable entity reference with world access
    /// * `component` - Python object to convert and insert
    ///
    /// # Errors
    ///
    /// Returns error if Python conversion or insertion fails.
    fn insert_into_entity(
        &self,
        entity: &mut EntityWorldMut,
        component: &Bound<PyAny>,
    ) -> PyResult<()>;

    /// Convert one Python component into an owned uniform insertion payload.
    fn prepare_uniform(
        &self,
        _component: &Bound<PyAny>,
    ) -> PyResult<Box<dyn PreparedUniformComponent>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(format!(
            "{} cannot be spawned from Python",
            self.name()
        )))
    }

    /// Check if entity has this component type
    fn entity_contains(&self, entity: &EntityRef) -> bool;

    /// Extract a component as a re-resolving Python handle (read-only access).
    ///
    /// Used by `world.get()`. The returned wrapper caches no pointer: it re-derives
    /// the component's address from `(world_ptr, entity_id)` on each access, so it stays
    /// valid across structural mutations that relocate the component and errors after the
    /// entity is despawned. Returns `None` if the entity lacks this component.
    ///
    /// # Safety
    ///
    /// `world_ptr` must be valid while `validity` is non-Invalid, and must not have a
    /// competing mutable borrow for the duration of this call.
    unsafe fn extract_from_entity_ref(
        &self,
        entity_id: Entity,
        world_ptr: *mut World,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>>;

    /// Extract a component as a re-resolving Python handle (mutable access).
    ///
    /// Used by `world.get_mut()`. Like [`extract_from_entity_ref`](Self::extract_from_entity_ref)
    /// but `validity` carries write access, so mutations land on the live component.
    /// Returns `None` if the entity lacks this component.
    ///
    /// # Safety
    ///
    /// `world_ptr` must be valid while `validity` is non-Invalid, and must not have a
    /// competing mutable borrow for the duration of this call.
    unsafe fn extract_from_entity_mut(
        &self,
        entity_id: Entity,
        world_ptr: *mut World,
        validity: ValidityFlagWithMode,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>>;

    /// Insert the same component into multiple entities (bulk uniform spawn).
    ///
    /// Default loops `insert_into_entity`. Macro-generated bridges override
    /// this to extract the Python value once and clone in pure Rust.
    fn insert_bulk_uniform(
        &self,
        component: &Bound<PyAny>,
        entities: &[Entity],
        world: &mut World,
    ) -> PyResult<()> {
        for &entity_id in entities {
            let mut entity = world.entity_mut(entity_id);
            self.insert_into_entity(&mut entity, component)?;
        }
        Ok(())
    }

    /// Get View API bridge for this component (optional).
    ///
    /// Returns `Some(ViewBridge)` if this component supports the View API,
    /// which enables batch field access for performance-critical operations.
    ///
    /// Default implementation returns `None` (View API not supported).
    fn view_bridge(&self) -> Option<ViewBridge> {
        None
    }
}
