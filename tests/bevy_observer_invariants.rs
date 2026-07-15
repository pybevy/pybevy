//! Bevy 0.19 observer invariants relevant to PyBevy lifecycle execution.
//!
//! These tests preserve the durable findings from the native lifecycle
//! feasibility spike. They do not connect Bevy observers to Python or PyBevy's
//! production observer registry.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{
        component::ComponentId,
        event::{EntityComponentsTrigger, EventKey},
        lifecycle::{ADD, DESPAWN, DISCARD, INSERT, REMOVE},
        observer::{ObserverRunner, TriggerContext},
        ptr::PtrMut,
        world::DeferredWorld,
    },
    prelude::*,
};

#[derive(Component, Debug)]
struct LifecycleA(u32);

#[derive(Component, Debug)]
struct LifecycleB;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LifecycleRecord {
    event: &'static str,
    target: Entity,
    trigger_components: Vec<ComponentId>,
    a_value: Option<u32>,
    target_exists: bool,
}

#[derive(Component)]
struct RecordState(Arc<Mutex<Vec<LifecycleRecord>>>);

fn lifecycle_name(event_key: EventKey) -> &'static str {
    if event_key == ADD {
        "add"
    } else if event_key == INSERT {
        "insert"
    } else if event_key == DISCARD {
        "discard"
    } else if event_key == REMOVE {
        "remove"
    } else if event_key == DESPAWN {
        "despawn"
    } else {
        panic!("observer invariant runner received a non-lifecycle event key")
    }
}

/// Reads the target from erased Bevy lifecycle event data.
///
/// # Safety
/// `event` must be the event value identified by `event_key`.
unsafe fn lifecycle_target(event_key: EventKey, event: &PtrMut<'_>) -> Entity {
    if event_key == ADD {
        // SAFETY: the ADD event key guarantees an Add value.
        unsafe { event.as_ref().deref::<Add>() }.entity
    } else if event_key == INSERT {
        // SAFETY: the INSERT event key guarantees an Insert value.
        unsafe { event.as_ref().deref::<Insert>() }.entity
    } else if event_key == DISCARD {
        // SAFETY: the DISCARD event key guarantees a Discard value.
        unsafe { event.as_ref().deref::<Discard>() }.entity
    } else if event_key == REMOVE {
        // SAFETY: the REMOVE event key guarantees a Remove value.
        unsafe { event.as_ref().deref::<Remove>() }.entity
    } else if event_key == DESPAWN {
        // SAFETY: the DESPAWN event key guarantees a Despawn value.
        unsafe { event.as_ref().deref::<Despawn>() }.entity
    } else {
        panic!("observer invariant runner received a non-lifecycle event key")
    }
}

fn dynamic_lifecycle_observer(
    runner: ObserverRunner,
    event_key: EventKey,
    components: impl IntoIterator<Item = ComponentId>,
    target: Option<Entity>,
) -> Observer {
    // SAFETY: each caller pairs a built-in lifecycle key with a runner that
    // casts the erased event and trigger as that lifecycle family.
    let observer = unsafe {
        Observer::with_dynamic_runner(runner)
            .with_event_key(event_key)
            .with_components(components)
    };
    match target {
        Some(entity) => observer.with_entity(entity),
        None => observer,
    }
}

unsafe fn record_runner(
    mut world: DeferredWorld<'_>,
    observer: Entity,
    context: &TriggerContext,
    event: PtrMut<'_>,
    trigger: PtrMut<'_>,
) {
    // SAFETY: registration restricts this runner to lifecycle event keys.
    let target = unsafe { lifecycle_target(context.event_key, &event) };
    // SAFETY: every built-in lifecycle event uses EntityComponentsTrigger.
    let trigger = unsafe { trigger.as_ref().deref::<EntityComponentsTrigger<'_>>() };
    let records = {
        let state = world
            .get_mut::<RecordState>(observer)
            .expect("dynamic observer must carry RecordState");
        Arc::clone(&state.0)
    };
    let target_exists = world.get_entity_mut(target).is_ok();
    let a_value = world.get_mut::<LifecycleA>(target).map(|value| value.0);
    records
        .lock()
        .expect("record log lock poisoned")
        .push(LifecycleRecord {
            event: lifecycle_name(context.event_key),
            target,
            trigger_components: trigger.components.to_vec(),
            a_value,
            target_exists,
        });
}

#[test]
fn dynamic_runtime_ids_preserve_lifecycle_order_and_readability() {
    let mut world = World::new();
    let a_id = world.register_component::<LifecycleA>();
    let records = Arc::new(Mutex::new(Vec::new()));
    for event_key in [ADD, INSERT, DISCARD, REMOVE] {
        world.spawn((
            dynamic_lifecycle_observer(record_runner, event_key, [a_id], None),
            RecordState(Arc::clone(&records)),
        ));
    }

    let target = world.spawn_empty().id();
    world.entity_mut(target).insert(LifecycleA(1));
    world.entity_mut(target).insert(LifecycleA(2));
    world.entity_mut(target).remove::<LifecycleA>();

    let records = records.lock().expect("record log lock poisoned");
    assert_eq!(
        records
            .iter()
            .map(|record| (record.event, record.a_value))
            .collect::<Vec<_>>(),
        vec![
            ("add", Some(1)),
            ("insert", Some(1)),
            ("discard", Some(1)),
            ("insert", Some(2)),
            ("discard", Some(2)),
            ("remove", Some(2)),
        ]
    );
    assert!(records.iter().all(|record| record.target == target));
    assert!(records.iter().all(|record| record.target_exists));
    assert!(
        records
            .iter()
            .all(|record| record.trigger_components.contains(&a_id))
    );
    assert!(world.get::<LifecycleA>(target).is_none());
}

#[test]
fn targeted_despawn_runs_before_cleanup_and_auto_retires_observer() {
    let mut world = World::new();
    let a_id = world.register_component::<LifecycleA>();
    let records = Arc::new(Mutex::new(Vec::new()));
    let target = world.spawn(LifecycleA(7)).id();
    let observer = world
        .spawn((
            dynamic_lifecycle_observer(record_runner, DESPAWN, [a_id], Some(target)),
            RecordState(Arc::clone(&records)),
        ))
        .id();

    world.entity_mut(target).despawn();
    world.flush();

    let records = records.lock().expect("record log lock poisoned");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].event, "despawn");
    assert_eq!(records[0].a_value, Some(7));
    assert!(records[0].target_exists);
    assert!(world.get_entity(target).is_err());
    assert!(world.get_entity(observer).is_err());
}

#[derive(Component)]
struct RawCount(Arc<Mutex<usize>>);

unsafe fn raw_count_runner(
    mut world: DeferredWorld<'_>,
    observer: Entity,
    _context: &TriggerContext,
    _event: PtrMut<'_>,
    _trigger: PtrMut<'_>,
) {
    let count = {
        let state = world
            .get_mut::<RawCount>(observer)
            .expect("dynamic observer must carry RawCount");
        Arc::clone(&state.0)
    };
    *count.lock().expect("raw count lock poisoned") += 1;
}

#[derive(Component)]
struct DedupCount {
    last_trigger_id: Option<u32>,
    count: Arc<Mutex<usize>>,
}

unsafe fn dedup_count_runner(
    mut world: DeferredWorld<'_>,
    observer: Entity,
    _context: &TriggerContext,
    _event: PtrMut<'_>,
    _trigger: PtrMut<'_>,
) {
    let trigger_id = world.as_unsafe_world_cell().last_trigger_id();
    let count = {
        let mut state = world
            .get_mut::<DedupCount>(observer)
            .expect("dynamic observer must carry DedupCount");
        if state.last_trigger_id == Some(trigger_id) {
            return;
        }
        state.last_trigger_id = Some(trigger_id);
        Arc::clone(&state.count)
    };
    *count.lock().expect("dedup count lock poisoned") += 1;
}

#[test]
fn multi_component_dynamic_runner_requires_trigger_id_deduplication() {
    let mut world = World::new();
    let a_id = world.register_component::<LifecycleA>();
    let b_id = world.register_component::<LifecycleB>();
    let raw_count = Arc::new(Mutex::new(0));
    let dedup_count = Arc::new(Mutex::new(0));
    world.spawn((
        dynamic_lifecycle_observer(raw_count_runner, DESPAWN, [a_id, b_id], None),
        RawCount(Arc::clone(&raw_count)),
    ));
    world.spawn((
        dynamic_lifecycle_observer(dedup_count_runner, DESPAWN, [a_id, b_id], None),
        DedupCount {
            last_trigger_id: None,
            count: Arc::clone(&dedup_count),
        },
    ));

    let target = world.spawn((LifecycleA(9), LifecycleB)).id();
    world.entity_mut(target).despawn();

    assert_eq!(*raw_count.lock().expect("raw count lock poisoned"), 2);
    assert_eq!(*dedup_count.lock().expect("dedup lock poisoned"), 1);
}

#[derive(Component)]
struct DeferredReadState {
    immediate: Arc<Mutex<Vec<Option<u32>>>>,
    deferred: Arc<Mutex<Vec<Option<u32>>>>,
}

unsafe fn deferred_read_runner(
    mut world: DeferredWorld<'_>,
    observer: Entity,
    context: &TriggerContext,
    event: PtrMut<'_>,
    _trigger: PtrMut<'_>,
) {
    // SAFETY: registration restricts this runner to lifecycle event keys.
    let target = unsafe { lifecycle_target(context.event_key, &event) };
    let (immediate, deferred) = {
        let state = world
            .get_mut::<DeferredReadState>(observer)
            .expect("dynamic observer must carry DeferredReadState");
        (Arc::clone(&state.immediate), Arc::clone(&state.deferred))
    };
    immediate
        .lock()
        .expect("immediate log lock poisoned")
        .push(world.get_mut::<LifecycleA>(target).map(|value| value.0));
    world.commands().queue(move |world: &mut World| {
        deferred
            .lock()
            .expect("deferred log lock poisoned")
            .push(world.get::<LifecycleA>(target).map(|value| value.0));
    });
}

#[test]
fn deferred_world_reads_old_value_but_queued_world_runs_after_deletion() {
    let mut world = World::new();
    let a_id = world.register_component::<LifecycleA>();
    let immediate = Arc::new(Mutex::new(Vec::new()));
    let deferred = Arc::new(Mutex::new(Vec::new()));
    world.spawn((
        dynamic_lifecycle_observer(deferred_read_runner, REMOVE, [a_id], None),
        DeferredReadState {
            immediate: Arc::clone(&immediate),
            deferred: Arc::clone(&deferred),
        },
    ));
    let target = world.spawn(LifecycleA(41)).id();

    world.entity_mut(target).remove::<LifecycleA>();
    world.flush();

    assert_eq!(
        *immediate.lock().expect("immediate log lock poisoned"),
        vec![Some(41)]
    );
    assert_eq!(
        *deferred.lock().expect("deferred log lock poisoned"),
        vec![None]
    );
}

#[test]
fn native_observer_rejects_exclusive_world_parameter() {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Observer::new(|_: On<Add, LifecycleA>, _world: &mut World| {});
    }));
    assert!(result.is_err());
}

#[derive(Clone, Copy)]
enum QueueAction {
    InsertB,
    None,
}

#[derive(Component)]
struct QueueState {
    label: &'static str,
    action: QueueAction,
    log: Arc<Mutex<Vec<&'static str>>>,
}

unsafe fn queue_runner(
    mut world: DeferredWorld<'_>,
    observer: Entity,
    context: &TriggerContext,
    event: PtrMut<'_>,
    _trigger: PtrMut<'_>,
) {
    // SAFETY: registration restricts this runner to lifecycle event keys.
    let target = unsafe { lifecycle_target(context.event_key, &event) };
    let (label, action, log) = {
        let state = world
            .get_mut::<QueueState>(observer)
            .expect("dynamic observer must carry QueueState");
        (state.label, state.action, Arc::clone(&state.log))
    };
    log.lock().expect("queue log lock poisoned").push(label);
    if matches!(action, QueueAction::InsertB) {
        world.commands().entity(target).insert(LifecycleB);
    }
}

#[test]
fn observer_commands_flush_after_all_observers_for_current_trigger() {
    let mut world = World::new();
    let a_id = world.register_component::<LifecycleA>();
    let b_id = world.register_component::<LifecycleB>();
    let log = Arc::new(Mutex::new(Vec::new()));
    for (component, label, action) in [
        (a_id, "a-queues-b", QueueAction::InsertB),
        (a_id, "a-peer", QueueAction::None),
        (b_id, "b", QueueAction::None),
    ] {
        world.spawn((
            dynamic_lifecycle_observer(queue_runner, ADD, [component], None),
            QueueState {
                label,
                action,
                log: Arc::clone(&log),
            },
        ));
    }

    let target = world.spawn_empty().id();
    world.entity_mut(target).insert(LifecycleA(1));
    world.flush();

    let log = log.lock().expect("queue log lock poisoned");
    assert_eq!(log.len(), 3);
    let mut current_trigger = log[..2].to_vec();
    current_trigger.sort();
    assert_eq!(current_trigger, vec!["a-peer", "a-queues-b"]);
    assert_eq!(log[2], "b");
}
