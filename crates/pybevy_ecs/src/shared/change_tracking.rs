//! Change tracking for custom components.
//!
//! Implements lazy change detection that matches Bevy's semantics:
//! - Query[Mut[T]] iteration does NOT mark components as changed
//! - Only actual field mutations (via __setattr__) mark components as changed
//!
//! Safety: World pointer is only dereferenced during valid query iteration (protected by ValidityFlag)

use bevy::ecs::{
    change_detection::DetectChangesMut, component::ComponentId, entity::Entity, world::World,
};

/// Mark a specific component as changed using explicit entity and world pointer.
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

#[cfg(test)]
mod tests {
    use bevy::ecs::component::Component;

    use super::*;

    #[derive(Component)]
    struct Health;

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
        let ticks = world
            .entity(entity)
            .get_change_ticks_by_id(component_id)
            .unwrap();
        assert!(!ticks.is_changed(last_run, this_run));

        // Mark it
        let world_ptr: *mut World = &mut world;
        unsafe { mark_component_changed_explicit(entity, world_ptr, component_id) };

        // Now it should appear changed
        let this_run = world.read_change_tick();
        let ticks = world
            .entity(entity)
            .get_change_ticks_by_id(component_id)
            .unwrap();
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
