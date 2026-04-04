use bevy::ecs::world::{CommandQueue, unsafe_world_cell::UnsafeWorldCell};
use bevy::ecs::system::Commands;

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
