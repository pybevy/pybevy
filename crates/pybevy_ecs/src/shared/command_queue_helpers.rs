use bevy::ecs::{
    system::Commands,
    world::{CommandQueue, unsafe_world_cell::UnsafeWorldCell},
};

/// Create `Commands` from an existing `CommandQueue` and an `UnsafeWorldCell`.
///
/// This avoids requiring `&mut World` (which would force exclusive access),
/// using only read-only metadata (entities + allocator) from the world cell.
/// The queue should be flushed to the world via `queue.apply(world)` after
/// the system completes.
pub fn create_commands_from_queue<'a>(
    queue: &'a mut CommandQueue,
    world: UnsafeWorldCell<'a>,
) -> Commands<'a, 'a> {
    let allocator = world.entities_allocator();
    let entities = world.entities();
    Commands::new_from_entities(queue, allocator, entities)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::{component::Component, world::World};

    #[derive(Component)]
    struct Marker;

    #[test]
    fn commands_can_spawn_and_flush() {
        let mut world = World::new();
        let mut queue = CommandQueue::default();

        // Create commands and spawn an entity
        {
            let cell = world.as_unsafe_world_cell();
            let mut commands = create_commands_from_queue(&mut queue, cell);
            commands.spawn(Marker);
        }

        // Before flush, entity shouldn't exist yet
        assert_eq!(world.query::<&Marker>().iter(&world).count(), 0);

        // After flush, entity should exist
        queue.apply(&mut world);
        assert_eq!(world.query::<&Marker>().iter(&world).count(), 1);
    }
}
