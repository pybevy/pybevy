//! Lazy change tracking for borrowed native component wrappers.

use bevy::ecs::{
    change_detection::{DetectChangesMut, Tick},
    component::ComponentId,
    entity::Entity,
    world::unsafe_world_cell::UnsafeWorldCell,
};

use crate::StorageError;

/// ECS identity and tick window used to mark a native component on its first write.
#[derive(Clone, Copy, Debug)]
pub struct ComponentWriteContext {
    world: UnsafeWorldCell<'static>,
    entity: Entity,
    component_id: ComponentId,
    offset: usize,
    last_run: Tick,
    this_run: Tick,
}

impl ComponentWriteContext {
    /// Create a write context fenced by the borrowed wrapper's validity flag.
    ///
    /// # Safety
    /// - `world` must remain live while the associated validity flag is valid.
    /// - Scheduler access must permit mutable access to `component_id`.
    /// - `entity` and `component_id` must identify the component behind the wrapper.
    pub unsafe fn new(
        world: UnsafeWorldCell<'_>,
        entity: Entity,
        component_id: ComponentId,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        // SAFETY: forwards this constructor's contract with a whole-component offset.
        unsafe { Self::new_with_offset(world, entity, component_id, 0, last_run, this_run) }
    }

    /// Create a write context for a field at `offset` bytes into a component.
    ///
    /// # Safety
    /// The requirements of [`Self::new`] apply, and `offset` must identify a field
    /// within the registered component allocation.
    pub unsafe fn new_with_offset(
        world: UnsafeWorldCell<'_>,
        entity: Entity,
        component_id: ComponentId,
        offset: usize,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        // SAFETY: the caller fences this lifetime-erased cell with the same validity
        // flag carried by the borrowed component storage.
        let world =
            unsafe { std::mem::transmute::<UnsafeWorldCell<'_>, UnsafeWorldCell<'static>>(world) };
        Self {
            world,
            entity,
            component_id,
            offset,
            last_run,
            this_run,
        }
    }

    /// Derive a tracker for a nested field at `offset` bytes from this value.
    pub(crate) fn child(self, offset: usize) -> Self {
        Self {
            offset: self.offset + offset,
            ..self
        }
    }

    /// Resolve the current field address without marking the component changed.
    pub(crate) fn resolve(self) -> Result<*mut u8, StorageError> {
        let entity = self
            .world
            .get_entity(self.entity)
            .map_err(|_| StorageError::EntityUnavailable)?;
        // SAFETY: construction guarantees that this exact component is covered by
        // the query's declared access while the associated validity flag is live.
        let base = unsafe { entity.get_by_id(self.component_id) }
            .ok_or(StorageError::EntityUnavailable)?
            .as_ptr();
        // SAFETY: offset was measured from a field within this component.
        Ok(unsafe { base.add(self.offset) })
    }

    /// Mark the component changed and resolve the current writable field address.
    pub(crate) fn resolve_mut(self) -> Result<*mut u8, StorageError> {
        let entity = self
            .world
            .get_entity_with_ticks(self.entity, self.last_run, self.this_run)
            .map_err(|_| StorageError::EntityUnavailable)?;
        // SAFETY: construction requires declared mutable scheduler access to this
        // component, and the validity flag confines use to that system run.
        let mut component = unsafe { entity.get_mut_by_id(self.component_id) }
            .map_err(|_| StorageError::EntityUnavailable)?;
        component.set_changed();
        let base = component.bypass_change_detection().as_ptr();
        // SAFETY: offset was measured from a field within this component.
        Ok(unsafe { base.add(self.offset) })
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::{change_detection::DetectChangesMut, component::Component, world::World};

    use super::*;
    use crate::{AccessMode, ComponentStorage, ValidityFlag, ValueStorage};

    #[derive(Component)]
    struct NativeComponent {
        value: i32,
    }

    fn tracked_storage(
        world: &mut World,
        entity: Entity,
        component_id: ComponentId,
        last_run: Tick,
        this_run: Tick,
    ) -> ComponentStorage<NativeComponent> {
        let ptr = {
            let mut component = world.get_mut::<NativeComponent>(entity).unwrap();
            component.bypass_change_detection() as *mut NativeComponent
        };
        let world_cell = world.as_unsafe_world_cell();
        // SAFETY: the test keeps `world` alive and performs no competing component access
        // while the returned storage is used.
        let context = unsafe {
            ComponentWriteContext::new(world_cell, entity, component_id, last_run, this_run)
        };
        let validity = ValidityFlag::new_write()
            .with_access_mode(AccessMode::Write)
            .with_component_write_context(context);
        // SAFETY: ptr and context identify the same live component and the validity
        // flag remains active for the test.
        unsafe { ComponentStorage::borrowed(ptr, validity) }
    }

    fn is_changed(
        world: &World,
        entity: Entity,
        component_id: ComponentId,
        last_run: Tick,
        this_run: Tick,
    ) -> bool {
        world
            .entity(entity)
            .get_change_ticks_by_id(component_id)
            .unwrap()
            .is_changed(last_run, this_run)
    }

    #[test]
    fn read_does_not_mark_native_component_changed() {
        let mut world = World::new();
        let entity = world.spawn(NativeComponent { value: 7 }).id();
        let component_id = world
            .components()
            .component_id::<NativeComponent>()
            .unwrap();
        let last_run = world.read_change_tick();
        world.increment_change_tick();
        world.increment_change_tick();
        let this_run = world.read_change_tick();

        let storage = tracked_storage(&mut world, entity, component_id, last_run, this_run);
        assert_eq!(storage.as_ref().unwrap().value, 7);
        assert!(!is_changed(
            &world,
            entity,
            component_id,
            last_run,
            this_run
        ));
    }

    #[test]
    fn nested_write_marks_native_component_changed() {
        let mut world = World::new();
        let entity = world.spawn(NativeComponent { value: 7 }).id();
        let component_id = world
            .components()
            .component_id::<NativeComponent>()
            .unwrap();
        let last_run = world.read_change_tick();
        world.increment_change_tick();
        world.increment_change_tick();
        let this_run = world.read_change_tick();

        let storage = tracked_storage(&mut world, entity, component_id, last_run, this_run);
        let mut value: ValueStorage<i32> = storage.borrow_field(|value| &value.value).unwrap();
        assert_eq!(value.get().unwrap(), 7);
        assert!(!is_changed(
            &world,
            entity,
            component_id,
            last_run,
            this_run
        ));

        *value.as_mut().unwrap() = 9;
        assert_eq!(world.get::<NativeComponent>(entity).unwrap().value, 9);
        assert!(is_changed(&world, entity, component_id, last_run, this_run));
    }
}
