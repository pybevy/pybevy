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

use bevy::ecs::{
    change_detection::DetectChangesMut, component::ComponentId, entity::Entity, world::World,
};

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
            if let Ok(mut comp) = entity_mut.get_mut_by_id(component_id) {
                comp.set_changed();
            }
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

/// Read the current entity context (test-only).
#[cfg(test)]
fn get_entity_context() -> (Option<Entity>, Option<*mut World>) {
    let entity = CURRENT_ENTITY.with(|e| e.get());
    let world_ptr = WORLD_PTR.with(|w| w.get());
    (entity, world_ptr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::component::Component;

    #[derive(Component)]
    struct Health;

    #[test]
    fn context_initially_empty() {
        clear_entity_context();
        let (entity, world_ptr) = get_entity_context();
        assert!(entity.is_none());
        assert!(world_ptr.is_none());
    }

    #[test]
    fn set_and_clear_context() {
        let entity = Entity::from_bits(42);
        set_entity_context(entity, std::ptr::null_mut());

        let (e, w) = get_entity_context();
        assert_eq!(e, Some(entity));
        assert!(w.is_some());

        clear_entity_context();
        let (e, w) = get_entity_context();
        assert!(e.is_none());
        assert!(w.is_none());
    }

    #[test]
    fn set_overwrites_previous_context() {
        let a = Entity::from_bits(10);
        let b = Entity::from_bits(20);

        set_entity_context(a, std::ptr::null_mut());
        set_entity_context(b, std::ptr::null_mut());

        let (e, _) = get_entity_context();
        assert_eq!(e, Some(b));

        clear_entity_context();
    }

    #[test]
    fn clear_is_idempotent() {
        clear_entity_context();
        clear_entity_context();
        let (e, _) = get_entity_context();
        assert!(e.is_none());
    }

    #[test]
    fn mark_changed_updates_change_tick() {
        let mut world = World::new();
        let component_id = world.register_component::<Health>();
        let entity = world.spawn(Health).id();

        // Advance change ticks so the initial "added" change is in the past
        let last_run = world.read_change_tick();
        world.increment_change_tick();
        world.increment_change_tick();

        // Before marking, component should not appear changed since last_run
        let this_run = world.read_change_tick();
        let ticks = world.entity(entity).get_change_ticks_by_id(component_id).unwrap();
        assert!(!ticks.is_changed(last_run, this_run));

        // Mark it
        let world_ptr: *mut World = &mut world;
        unsafe { mark_component_changed_explicit(entity, world_ptr, component_id) };

        // Now it should appear changed
        let this_run = world.read_change_tick();
        let ticks = world.entity(entity).get_change_ticks_by_id(component_id).unwrap();
        assert!(
            ticks.is_changed(last_run, this_run),
            "component should be marked as changed after mark_component_changed_explicit"
        );
    }

    #[test]
    fn mark_changed_nonexistent_entity_does_not_panic() {
        let mut world = World::new();
        let component_id = world.register_component::<Health>();
        let entity = Entity::from_bits(9999);

        let world_ptr: *mut World = &mut world;
        // Should silently do nothing
        unsafe { mark_component_changed_explicit(entity, world_ptr, component_id) };
    }

    #[test]
    fn mark_changed_missing_component_does_not_panic() {
        let mut world = World::new();
        let component_id = world.register_component::<Health>();
        // Spawn entity without Health
        let entity = world.spawn_empty().id();

        let world_ptr: *mut World = &mut world;
        // Should silently do nothing
        unsafe { mark_component_changed_explicit(entity, world_ptr, component_id) };
    }
}

