//! Thread-local change tracking for custom components with PyObject storage.
//!
//! This module implements lazy change detection that matches Bevy's semantics:
//! - Query[Mut[T]] iteration does NOT mark components as changed
//! - Only actual field mutations (via __setattr__) mark components as changed
//!
//! Architecture:
//! 1. When query iteration starts for an entity, set_entity_context() stores entity + world_ptr
//! 2. When __setattr__ is called, it calls mark_component_changed() with the component_id
//! 3. mark_component_changed() immediately marks that specific component as changed
//!
//! Safety: World pointer is only dereferenced during valid query iteration (protected by ValidityFlag)

use std::cell::Cell;

use bevy::ecs::{component::ComponentId, entity::Entity, world::World};

thread_local! {
    /// Current entity being accessed during query iteration
    static CURRENT_ENTITY: Cell<Option<Entity>> = const { Cell::new(None) };

    /// World pointer valid during query iteration
    static WORLD_PTR: Cell<Option<*mut World>> = const { Cell::new(None) };
}

/// Set the entity context at the start of processing an entity in a query.
///
/// This must be called when starting to process each entity in a query iteration.
/// It stores the entity and world pointer for use by mark_component_changed().
///
/// # Safety
/// - world_ptr must be valid for the duration of processing this entity
/// - Must call clear_entity_context() when done with this entity
pub fn set_entity_context(entity: Entity, world_ptr: *mut World) {
    CURRENT_ENTITY.with(|e| e.set(Some(entity)));
    WORLD_PTR.with(|w| w.set(Some(world_ptr)));
}

/// Mark a specific component as changed using explicit entity and world pointer.
///
/// Unlike `mark_component_changed()`, this does not rely on the thread-local context.
/// This allows change detection to work for components accessed outside the iteration
/// loop (e.g., items collected via `list(query)`).
///
/// # Safety
/// - `world_ptr` must point to a valid `World` (caller must ensure the pointer
///   has not been invalidated, e.g. via `ValidityFlag`).
/// - `entity` must exist in the world.
/// - No other mutable reference to the `World` may be live at the call site.
pub unsafe fn mark_component_changed_explicit(
    entity: Entity,
    world_ptr: *mut World,
    component_id: ComponentId,
) {
    // SAFETY:
    // - world_ptr is valid because ValidityFlag is still active
    // - Entity and ComponentId are valid (came from query extraction)
    unsafe {
        let world = &mut *world_ptr;
        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
            // Mark component as changed by getting mutable access
            // get_mut_by_id() triggers Bevy's DerefMut, which calls set_changed()
            let _ = entity_mut.get_mut_by_id(component_id);
        }
    }
}

/// Clear the entity context.
///
/// This must be called when done processing an entity.
pub fn clear_entity_context() {
    CURRENT_ENTITY.with(|e| e.set(None));
    WORLD_PTR.with(|w| w.set(None));
}
