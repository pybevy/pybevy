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
        if let Ok(mut entity_mut) = world.get_entity_mut(entity)
            && let Ok(mut comp) = entity_mut.get_mut_by_id(component_id)
        {
            comp.set_changed();
        }
    }
}

/// Re-resolve the current data pointer for a wrapper-storage custom component.
///
/// Borrowed wrapper proxies (LazyWrapperProxy) cache a raw pointer
/// into ECS table storage at construction. A structural world mutation
/// (spawn/despawn/insert/remove) can swap-remove the entity's row, move the entity to
/// a different archetype table, or reallocate the column, leaving that cached pointer
/// dangling. Rather than trust the cached pointer, long-lived proxies (returned from
/// `world.get` / `world.get_mut`) call this on every field access to fetch the
/// component's CURRENT address, or `None` if the entity was despawned or the component
/// removed since the proxy was created.
///
/// The returned pointer aliases the component's first byte. Wrapper component structs
/// are `#[repr(C)]` with `data: [u8; N]` as their sole field at offset 0, so the
/// component base pointer is also the data-array pointer the proxy reads and writes.
///
/// This mirrors the derivation at proxy construction (an immutable `get_by_id` whose
/// const pointer is cast to `*mut u8`); write-back change tracking stays separate, via
/// `mark_component_changed_explicit`, so no `&mut World` and no read-time change
/// detection is involved here.
///
/// # Safety
/// - `world_ptr` must point to a valid `World` (protected by `ValidityFlag`).
/// - No other mutable reference to the `World` may be live at the call site.
/// - The returned pointer is invalidated by the next structural mutation; the caller
///   must not hold it across one (re-resolve again instead).
pub unsafe fn reresolve_wrapper_ptr(
    entity: Entity,
    world_ptr: *mut World,
    component_id: ComponentId,
) -> Option<*mut u8> {
    // SAFETY: caller guarantees world_ptr validity and the absence of a competing
    // mutable borrow. A shared `&World` is sufficient: `get_by_id` yields a const
    // `Ptr`, matching how the proxy's pointer is derived at construction.
    unsafe {
        let world = &*world_ptr;
        let entity_ref = world.get_entity(entity).ok()?;
        let ptr = entity_ref.get_by_id(component_id).ok()?;
        Some(ptr.as_ptr())
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

    // A wrapper-like component: repr(C) with the data payload at offset 0, matching
    // how real wrapper components (ComponentWrapperN) are laid out.
    #[repr(C, align(8))]
    #[derive(Component)]
    struct Pos {
        x: f32,
    }

    #[test]
    fn reresolve_returns_live_component_pointer() {
        let mut world = World::new();
        let component_id = world.register_component::<Pos>();
        let entity = world.spawn(Pos { x: 1.5 }).id();

        // The address the proxy would have cached at construction.
        let expected = world
            .entity(entity)
            .get_by_id(component_id)
            .unwrap()
            .as_ptr();

        let world_ptr: *mut World = &mut world;
        let resolved = unsafe { reresolve_wrapper_ptr(entity, world_ptr, component_id) };
        assert_eq!(resolved, Some(expected));

        // The pointer reads the field back correctly (offset 0).
        let x = unsafe { *(resolved.unwrap() as *const f32) };
        assert_eq!(x, 1.5);
    }

    #[test]
    fn reresolve_returns_none_after_despawn() {
        let mut world = World::new();
        let component_id = world.register_component::<Pos>();
        let entity = world.spawn(Pos { x: 2.0 }).id();
        world.despawn(entity);

        let world_ptr: *mut World = &mut world;
        let resolved = unsafe { reresolve_wrapper_ptr(entity, world_ptr, component_id) };
        assert_eq!(resolved, None);
    }

    #[test]
    fn reresolve_returns_none_for_missing_component() {
        let mut world = World::new();
        let component_id = world.register_component::<Pos>();
        // Entity exists but lacks the component.
        let entity = world.spawn_empty().id();

        let world_ptr: *mut World = &mut world;
        let resolved = unsafe { reresolve_wrapper_ptr(entity, world_ptr, component_id) };
        assert_eq!(resolved, None);
    }

    #[test]
    fn reresolve_tracks_pointer_across_archetype_move() {
        // The core value of re-resolution: after a structural mutation moves the
        // entity to a new archetype table, a cached pointer would dangle, but
        // re-resolving returns the component's NEW valid address.
        let mut world = World::new();
        let pos_id = world.register_component::<Pos>();
        let entity = world.spawn(Pos { x: 3.25 }).id();

        let stale = world.entity(entity).get_by_id(pos_id).unwrap().as_ptr();

        // Insert a second component: structural move to a new archetype/table.
        world.entity_mut(entity).insert(Health);

        let world_ptr: *mut World = &mut world;
        let resolved =
            unsafe { reresolve_wrapper_ptr(entity, world_ptr, pos_id) }.expect("still present");

        // Reading through the re-resolved pointer yields the preserved value,
        // regardless of whether the underlying storage address changed.
        let x = unsafe { *(resolved as *const f32) };
        assert_eq!(x, 3.25);
        // Sanity: the resolved pointer is the one the world reports now, which may or
        // may not differ from the pre-move address depending on allocator reuse.
        let current = world.entity(entity).get_by_id(pos_id).unwrap().as_ptr();
        assert_eq!(resolved, current);
        let _ = stale;
    }
}
