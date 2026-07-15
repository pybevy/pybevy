//! Bevy 0.19 batch-spawn invariants relevant to `BatchSpawnCore`.
//!
//! These tests characterize native Bevy behavior before PyBevy extracts its
//! structural batch pipeline. They intentionally test Bevy directly rather
//! than PyBevy's current Python adapters.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use bevy::{ecs::world::DeferredWorld, prelude::*};

#[derive(Component, Debug, PartialEq, Eq)]
struct BatchValue(u32);

#[derive(Component, Debug, PartialEq, Eq)]
struct HookInserted;

#[derive(Component, Debug, PartialEq, Eq)]
struct RecursivelySpawned(Entity);

#[derive(Resource, Clone)]
struct HookLog(Arc<Mutex<Vec<(&'static str, Entity)>>>);

fn push_log(world: &mut DeferredWorld<'_>, label: &'static str, entity: Entity) {
    let log = Arc::clone(&world.resource::<HookLog>().0);
    log.lock()
        .expect("batch hook log lock poisoned")
        .push((label, entity));
}

#[test]
fn spawn_batch_rejects_duplicate_component_types_before_allocating_entities() {
    let mut world = World::new();

    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = world.spawn_batch([(BatchValue(1), BatchValue(2))]);
    }));

    assert!(result.is_err());
    assert_eq!(world.query::<&BatchValue>().iter(&world).count(), 0);
}

#[test]
fn spawn_batch_runs_inline_hooks_per_entity_then_flushes_their_commands_on_drop() {
    let mut world = World::new();
    let log = Arc::new(Mutex::new(Vec::new()));
    world.insert_resource(HookLog(Arc::clone(&log)));
    world
        .register_component_hooks::<BatchValue>()
        .on_add(|mut world, context| {
            push_log(&mut world, "value-add", context.entity);
            world.commands().entity(context.entity).insert(HookInserted);
        });
    world
        .register_component_hooks::<HookInserted>()
        .on_add(|mut world, context| {
            push_log(&mut world, "queued-insert-add", context.entity);
        });

    let entities = world
        .spawn_batch([BatchValue(1), BatchValue(2), BatchValue(3)])
        .collect::<Vec<_>>();

    assert_eq!(entities.len(), 3);
    assert!(
        entities
            .iter()
            .all(|&entity| world.get::<HookInserted>(entity).is_some())
    );
    assert_eq!(
        *log.lock().expect("batch hook log lock poisoned"),
        vec![
            ("value-add", entities[0]),
            ("value-add", entities[1]),
            ("value-add", entities[2]),
            ("queued-insert-add", entities[0]),
            ("queued-insert-add", entities[1]),
            ("queued-insert-add", entities[2]),
        ]
    );
}

#[test]
fn dropping_a_partially_consumed_batch_spawns_the_remaining_entities() {
    let mut world = World::new();
    let first;
    {
        let mut batch = world.spawn_batch([BatchValue(1), BatchValue(2), BatchValue(3)]);
        first = batch.next().expect("batch should yield its first entity");
        // `SpawnBatchIter::drop` consumes the two remaining input values.
    }

    let mut values = world
        .query::<(Entity, &BatchValue)>()
        .iter(&world)
        .map(|(entity, value)| (entity, value.0))
        .collect::<Vec<_>>();
    values.sort_by_key(|(_, value)| *value);

    assert_eq!(values.len(), 3);
    assert_eq!(values[0], (first, 1));
    assert_eq!(
        values.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

#[test]
fn hook_commands_can_remove_batch_components_and_despawn_returned_entities() {
    let mut world = World::new();
    world
        .register_component_hooks::<BatchValue>()
        .on_add(|mut world, context| {
            let value = world
                .get::<BatchValue>(context.entity)
                .expect("on_add must see inserted batch data")
                .0;
            if value == 1 {
                world
                    .commands()
                    .entity(context.entity)
                    .remove::<BatchValue>();
            } else {
                world.commands().entity(context.entity).despawn();
            }
        });

    let entities = world
        .spawn_batch([BatchValue(1), BatchValue(2)])
        .collect::<Vec<_>>();

    assert!(world.get_entity(entities[0]).is_ok());
    assert!(world.get::<BatchValue>(entities[0]).is_none());
    assert!(world.get_entity(entities[1]).is_err());
}

#[test]
fn hook_commands_can_recursively_spawn_entities_during_batch_flush() {
    let mut world = World::new();
    world
        .register_component_hooks::<BatchValue>()
        .on_add(|mut world, context| {
            world.commands().spawn(RecursivelySpawned(context.entity));
        });

    let entities = world
        .spawn_batch([BatchValue(1), BatchValue(2)])
        .collect::<Vec<_>>();
    let mut parents = world
        .query::<&RecursivelySpawned>()
        .iter(&world)
        .map(|spawned| spawned.0)
        .collect::<Vec<_>>();
    parents.sort();
    let mut expected = entities.clone();
    expected.sort();

    assert_eq!(parents, expected);
    assert_eq!(world.query::<&BatchValue>().iter(&world).count(), 2);
    assert_eq!(world.query::<&RecursivelySpawned>().iter(&world).count(), 2);
}

#[test]
fn native_hook_panic_leaves_the_current_batch_entity_allocated() {
    let mut world = World::new();
    world
        .register_component_hooks::<BatchValue>()
        .on_add(|_, _| panic!("intentional batch hook panic"));

    // Keep this to one item: `SpawnBatchIter::drop` consumes remaining items,
    // so another panicking item during unwinding would cause a double panic.
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = world.spawn_batch([BatchValue(1)]).collect::<Vec<_>>();
    }));

    assert!(result.is_err());
    let values = world.query::<&BatchValue>().iter(&world).count();
    assert_eq!(values, 1);
}
