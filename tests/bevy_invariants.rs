//! Bevy ECS Invariant Tests
//!
//! These tests verify the Bevy ECS guarantees that PyBevy's system-scoped
//! validity implementation depends on. If any of these tests fail on a new
//! Bevy version, the Query implementation needs to be reevaluated.
//!
//! See: docs/safety.md

use std::any::TypeId;

use bevy::{
    ecs::query::{FilteredAccess, FilteredAccessSet},
    prelude::*,
    state::app::StatesPlugin,
};

#[derive(Component)]
struct TestComponent {
    value: f32,
}

#[derive(Component)]
struct Marker;

#[derive(States, Clone, Debug, Default, Eq, Hash, PartialEq)]
enum TransitionContractState {
    #[default]
    Alpha,
    Beta,
}

#[derive(Resource, Debug, Default)]
struct TransitionContractTrace(Vec<(&'static str, TransitionContractState)>);

fn record_contract_exit(
    state: Res<State<TransitionContractState>>,
    mut trace: ResMut<TransitionContractTrace>,
) {
    trace.0.push(("exit", state.get().clone()));
}

fn record_contract_transition(
    state: Res<State<TransitionContractState>>,
    mut trace: ResMut<TransitionContractTrace>,
) {
    trace.0.push(("transition", state.get().clone()));
}

fn record_contract_enter(
    state: Res<State<TransitionContractState>>,
    mut trace: ResMut<TransitionContractTrace>,
) {
    trace.0.push(("enter", state.get().clone()));
}

fn record_contract_initial_enter(mut trace: ResMut<TransitionContractTrace>) {
    trace
        .0
        .push(("initial_enter", TransitionContractState::Alpha));
}

#[test]
fn test_query_yields_unique_entities() {
    //! INVARIANT: QueryState::iter_mut() yields each entity exactly once.
    //!
    //! If this fails: Multiple mutable references to same component possible.
    //! Impact: Need per-iteration validity or QueryOwned.

    let mut world = World::new();

    // Spawn 100 entities
    for i in 0..100 {
        world.spawn(TestComponent { value: i as f32 });
    }

    let mut query_state = world.query::<(Entity, &TestComponent)>();
    let mut seen_entities = std::collections::HashSet::new();
    let mut entity_count = 0;

    // Iterate and track entity IDs
    for (entity, _component) in query_state.iter(&world) {
        assert!(
            !seen_entities.contains(&entity),
            "Query yielded duplicate entity: {:?}",
            entity
        );
        seen_entities.insert(entity);
        entity_count += 1;
    }

    assert_eq!(
        entity_count, 100,
        "Expected 100 entities, got {}",
        entity_count
    );
    assert_eq!(
        seen_entities.len(),
        100,
        "Expected 100 unique entities, got {}",
        seen_entities.len()
    );
}

#[test]
fn test_query_iter_mut_yields_unique_mutable_references() {
    //! INVARIANT: QueryState::iter_mut() provides exclusive mutable access.
    //!
    //! This is the core safety guarantee. Each component should be yielded
    //! exactly once with mutable access.

    let mut world = World::new();

    // Spawn entities with values 0.0 to 9.0
    for i in 0..10 {
        world.spawn(TestComponent { value: i as f32 });
    }

    let mut query_state = world.query::<&mut TestComponent>();

    // Modify all components
    for mut component in query_state.iter_mut(&mut world) {
        component.value += 100.0;
    }

    // Verify all modifications were applied (no overwrites)
    let mut values: Vec<f32> = query_state.iter(&world).map(|c| c.value).collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let expected: Vec<f32> = (0..10).map(|i| i as f32 + 100.0).collect();
    assert_eq!(
        values, expected,
        "Components were not uniquely modified. Got {:?}, expected {:?}",
        values, expected
    );
}

#[test]
fn test_commands_are_deferred_during_iteration() {
    //! INVARIANT: Structural changes (spawn/despawn) are deferred until after system.
    //!
    //! If this fails: Archetypes can change during iteration, invalidating pointers.
    //! Impact: Need immediate cloning or QueryOwned.

    let mut world = World::new();

    // Spawn 10 initial entities
    for i in 0..10 {
        world.spawn(TestComponent { value: i as f32 });
    }

    // System that spawns entities during query iteration
    fn test_system(
        query: Query<&TestComponent>,
        mut commands: Commands,
        mut counter: ResMut<IterationCounter>,
    ) {
        for _component in query.iter() {
            counter.count += 1;
            // Spawn new entity during iteration
            commands.spawn(TestComponent { value: 999.0 });
        }
    }

    #[derive(Resource)]
    struct IterationCounter {
        count: usize,
    }

    world.insert_resource(IterationCounter { count: 0 });

    // Create and run the system
    let mut system = IntoSystem::into_system(test_system);
    system.initialize(&mut world);
    system.run((), &mut world).expect("system should run");

    // Count should still be 10 (commands not yet applied)
    let counter = world.resource::<IterationCounter>();
    let count_during_iteration = counter.count;
    assert_eq!(
        count_during_iteration, 10,
        "Expected 10 iterations (commands deferred), got {}",
        count_during_iteration
    );

    // Apply deferred commands
    system.apply_deferred(&mut world);

    // Now we should have 20 entities
    let final_count = world.query::<&TestComponent>().iter(&world).count();
    assert_eq!(
        final_count, 20,
        "Expected 20 entities after applying commands, got {}",
        final_count
    );
}

#[test]
fn test_exclusive_component_access_no_aliasing() {
    //! INVARIANT: Query has exclusive mutable access during iteration.
    //!
    //! If this fails: Concurrent modification possible.
    //! Impact: Need synchronization or ownership model.

    let mut world = World::new();

    // Spawn entities
    for i in 0..5 {
        world.spawn(TestComponent { value: i as f32 });
    }

    let mut query_state = world.query::<&mut TestComponent>();

    // Collect all mutable references
    let mut components: Vec<Mut<TestComponent>> = query_state.iter_mut(&mut world).collect();

    // Modify through the references
    for (i, component) in components.iter_mut().enumerate() {
        component.value = (i * 1000) as f32;
    }

    // Drop the mutable references
    drop(components);

    // Verify modifications were applied
    let values: Vec<f32> = query_state.iter(&world).map(|c| c.value).collect();

    let mut sorted_values = values.clone();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let expected: Vec<f32> = (0..5).map(|i| (i * 1000) as f32).collect();
    assert_eq!(
        sorted_values, expected,
        "Exclusive access was violated. Got {:?}, expected {:?}",
        sorted_values, expected
    );
}

#[test]
fn test_query_iteration_stable_across_multiple_iterations() {
    //! BEHAVIOR: Multiple iterations over the same query should be consistent.
    //!
    //! This tests the behavior that PyBevy's list(query) pattern depends on.

    let mut world = World::new();

    // Spawn entities with known values
    let values = [50.0, 10.0, 90.0, 30.0, 70.0];
    for &value in &values {
        world.spawn(TestComponent { value });
    }

    let mut query_state = world.query::<&TestComponent>();

    // First iteration: collect all values
    let first_pass: Vec<f32> = query_state.iter(&world).map(|c| c.value).collect();

    // Second iteration: should get same values
    let second_pass: Vec<f32> = query_state.iter(&world).map(|c| c.value).collect();

    // Third iteration: verify again
    let third_pass: Vec<f32> = query_state.iter(&world).map(|c| c.value).collect();

    assert_eq!(first_pass, second_pass, "First and second iteration differ");
    assert_eq!(second_pass, third_pass, "Second and third iteration differ");

    // Verify we got all expected values (order may vary)
    let mut sorted_first = first_pass.clone();
    sorted_first.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut expected = values.to_vec();
    expected.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert_eq!(
        sorted_first, expected,
        "Values don't match expected. Got {:?}, expected {:?}",
        sorted_first, expected
    );
}

#[test]
fn test_query_with_collect_preserves_mutability() {
    //! BEHAVIOR: Collecting query results into a Vec should preserve mutability.
    //!
    //! This is what PyBevy's list(query) does - it should allow subsequent
    //! modification of all items.

    let mut world = World::new();

    // Spawn entities
    for i in 0..5 {
        world.spawn(TestComponent { value: i as f32 });
    }

    let mut query_state = world.query::<&mut TestComponent>();

    // Collect into Vec (like Python's list())
    let mut collected: Vec<Mut<TestComponent>> = query_state.iter_mut(&mut world).collect();

    // Should be able to iterate and modify all items
    for component in &mut collected {
        component.value += 1000.0;
    }

    // Verify modifications
    let final_values: Vec<f32> = collected.iter().map(|c| c.value).collect();

    for (i, value) in final_values.iter().enumerate() {
        let expected = (i as f32) + 1000.0;
        assert!(
            (*value - expected).abs() < 0.001,
            "Component {} has value {}, expected {}",
            i,
            value,
            expected
        );
    }
}

#[test]
fn test_query_filter_preserves_uniqueness() {
    //! INVARIANT: Filtered queries still yield unique entities.

    let mut world = World::new();

    // Spawn entities, some with marker
    for i in 0..10 {
        let mut entity = world.spawn(TestComponent { value: i as f32 });
        if i % 2 == 0 {
            entity.insert(Marker);
        }
    }

    let mut query_state = world.query_filtered::<(Entity, &TestComponent), With<Marker>>();
    let mut seen_entities = std::collections::HashSet::new();

    for (entity, _component) in query_state.iter(&world) {
        assert!(
            !seen_entities.contains(&entity),
            "Filtered query yielded duplicate entity"
        );
        seen_entities.insert(entity);
    }

    // Should have seen exactly 5 entities (even indices)
    assert_eq!(seen_entities.len(), 5);
}

#[test]
fn test_world_archetype_stability_during_query() {
    //! INVARIANT: Query iteration should not be affected by archetype structure.
    //!
    //! This verifies that the internal archetype representation doesn't cause
    //! duplicate or missing entities.

    let mut world = World::new();

    // Create entities in different archetypes
    world.spawn(TestComponent { value: 1.0 });
    world.spawn((TestComponent { value: 2.0 }, Marker));
    world.spawn(TestComponent { value: 3.0 });
    world.spawn((TestComponent { value: 4.0 }, Marker));
    world.spawn(TestComponent { value: 5.0 });

    let mut query_state = world.query::<&TestComponent>();

    // Should see all 5 entities exactly once
    let values: Vec<f32> = query_state.iter(&world).map(|c| c.value).collect();

    assert_eq!(values.len(), 5, "Should see 5 entities");

    let mut sorted_values = values.clone();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert_eq!(
        sorted_values,
        vec![1.0, 2.0, 3.0, 4.0, 5.0],
        "All entities should be present exactly once"
    );
}

#[test]
fn test_commands_flush_applies_all_changes() {
    //! INVARIANT: Flushing commands applies all structural changes atomically.
    //!
    //! Note: In Bevy, when commands are run through system.run(), they appear
    //! to be applied automatically. The key invariant is that during query
    //! iteration within a system, commands are deferred (tested separately).

    let mut world = World::new();

    fn spawn_system(mut commands: Commands) {
        for i in 0..10 {
            commands.spawn(TestComponent { value: i as f32 });
        }
    }

    // Before system: no entities
    let count_before = world.query::<&TestComponent>().iter(&world).count();
    assert_eq!(count_before, 0, "No entities before system runs");

    let mut system = IntoSystem::into_system(spawn_system);
    system.initialize(&mut world);
    system.run((), &mut world).expect("system should run");

    // After system.run(), commands may already be applied
    // The important invariant (tested elsewhere) is that they're deferred
    // DURING query iteration within a system.
    system.apply_deferred(&mut world);

    // After flush: all 10 entities present
    let count_after = world.query::<&TestComponent>().iter(&world).count();
    assert_eq!(count_after, 10, "All commands should be applied");
}

#[test]
fn test_no_entity_duplication_across_archetypes() {
    //! INVARIANT: Entities that change archetypes during deferred operations
    //! should still only appear once in subsequent queries.

    let mut world = World::new();

    // Spawn entities without Marker
    let entities: Vec<Entity> = (0..5)
        .map(|i| world.spawn(TestComponent { value: i as f32 }).id())
        .collect();

    #[derive(Resource)]
    #[allow(dead_code)]
    struct EntityList(Vec<Entity>);
    world.insert_resource(EntityList(entities.clone()));

    // System that adds Marker to entities (changes archetype)
    fn modify_system(query: Query<Entity, With<TestComponent>>, mut commands: Commands) {
        for entity in query.iter() {
            commands.entity(entity).insert(Marker);
        }
    }

    let mut system = IntoSystem::into_system(modify_system);
    system.initialize(&mut world);
    system.run((), &mut world).expect("system should run");
    system.apply_deferred(&mut world);

    // Query should still return each entity exactly once
    let mut query_state = world.query::<(Entity, &TestComponent)>();
    let mut seen = std::collections::HashSet::new();

    for (entity, _component) in query_state.iter(&world) {
        assert!(
            !seen.contains(&entity),
            "Entity appeared multiple times after archetype change"
        );
        seen.insert(entity);
    }

    assert_eq!(seen.len(), 5, "Should see all 5 entities exactly once");
}

#[test]
fn test_world_lifetime_not_iterator_lifetime() {
    //! CRITICAL: Items yielded by QueryIter are tied to world lifetime ('w),
    //! not iterator lifetime. This is the foundation for system-scoped validity.
    //!
    //! This test explicitly demonstrates that you can:
    //! 1. Create an iterator
    //! 2. Get items from it
    //! 3. DROP the iterator
    //! 4. Continue using the items safely
    //!
    //! This is exactly what PyBevy does with list(query).

    let mut world = World::new();

    // Spawn entities
    for i in 0..5 {
        world.spawn(TestComponent { value: i as f32 });
    }

    let mut query_state = world.query::<&mut TestComponent>();

    // Create iterator and get items
    let mut iter = query_state.iter_mut(&mut world);

    let mut item1 = iter.next().expect("First item should exist");
    let mut item2 = iter.next().expect("Second item should exist");

    // CRITICAL: Drop the iterator
    // If items were tied to iterator lifetime, this would be a compile error
    drop(iter);

    // Items are still valid! They're tied to the query_state's borrow of world,
    // not to the iterator.
    let old_value1 = item1.value;
    let old_value2 = item2.value;

    item1.value = 100.0;
    item2.value = 200.0;

    assert_eq!(item1.value, 100.0);
    assert_eq!(item2.value, 200.0);

    // This is the EXACT pattern PyBevy uses:
    // 1. Create iterator (query.__iter__())
    // 2. Collect items (list(query))
    // 3. Iterator consumed/dropped
    // 4. Items still valid until system ends

    println!(
        "Successfully modified items after dropping iterator: {} -> {}, {} -> {}",
        old_value1, item1.value, old_value2, item2.value
    );
}

#[test]
fn test_collect_pattern_world_lifetime() {
    //! BEHAVIOR: The Vec::collect() pattern works because items are 'w lifetime.
    //!
    //! This is the Rust equivalent of Python's list(query).

    let mut world = World::new();

    // Spawn entities
    for i in 0..10 {
        world.spawn(TestComponent { value: i as f32 });
    }

    let mut query_state = world.query::<&mut TestComponent>();

    // Collect all items into a Vec
    // Iterator is consumed and dropped during collect()
    let mut items: Vec<Mut<TestComponent>> = query_state.iter_mut(&mut world).collect();

    // Items are still valid! Vec now holds all the mutable references.
    assert_eq!(items.len(), 10);

    // Modify all items
    for (i, item) in items.iter_mut().enumerate() {
        item.value = (i * 100) as f32;
    }

    // Verify modifications
    for (i, item) in items.iter().enumerate() {
        assert_eq!(
            item.value,
            (i * 100) as f32,
            "Item {} should have value {}",
            i,
            i * 100
        );
    }

    // This demonstrates that:
    // 1. collect() consumes the iterator
    // 2. Items remain valid in the Vec
    // 3. They're tied to the world borrow, not the iterator
    // 4. This is EXACTLY what Python's list(query) does
}

// ---------------------------------------------------------------------------
// Scheduler-access invariants
//
// These pin the FilteredAccess / FilteredAccessSet semantics that
// DynamicSystem::initialize (src/ecs/dynamic_system.rs) relies on when it
// declares access to Bevy's multithreaded executor. PyBevy hand-builds a
// FilteredAccessSet from parsed Python Query/View parameters; the executor's
// parallelism and conflict decisions depend on these rules holding exactly.
//
// If any of these fail on a new Bevy version, revisit build_full_access_set
// and the filter translation in query_filters_from_query_param /
// query_filters_from_view_param.
// ---------------------------------------------------------------------------

// Components used only by the scheduler-access invariant tests below.
// `SchedT` / `SchedU` stand in for two distinct queried components, and
// `SchedA` / `SchedB` stand in for filter-only marker components.
#[derive(Component)]
struct SchedT;

#[derive(Component)]
struct SchedU;

#[derive(Component)]
struct SchedA;

#[derive(Component)]
struct SchedB;

#[derive(Resource)]
struct SchedResource;

#[test]
fn test_filter_disjointness_write_with_vs_without() {
    //! INVARIANT: Two FilteredAccess values that both write the same component
    //! are compatible ONLY when their filters prove disjoint archetypes. With
    //! and_with(A) on one and and_without(A) on the other they are compatible;
    //! with and_with(A) on both they are not; with no extra filters they are
    //! not. (Note add_write(T) implicitly adds and_with(T), so "no filters"
    //! still means both carry with(T).)
    //!
    //! Relied on by: PyBevy translates Python `Without[...]` filters to
    //! and_without and `With[...]`/queried components to and_with; the
    //! executor's parallelism decisions hinge on this disjointness proof.
    //!
    //! Reevaluate if it fails: the Python filter-to-FilteredAccess translation
    //! in DynamicSystem::initialize would no longer describe when two systems
    //! may run in parallel, and could allow real data races or forbid safe ones.

    let mut world = World::new();
    let t = world.register_component::<SchedT>();
    let a = world.register_component::<SchedA>();

    // and_with(A) vs and_without(A): provably disjoint archetypes -> compatible.
    let mut with_a = FilteredAccess::default();
    with_a.add_write(t);
    with_a.and_with(a);
    let mut without_a = FilteredAccess::default();
    without_a.add_write(t);
    without_a.and_without(a);
    assert!(
        with_a.is_compatible(&without_a),
        "and_with(A) and and_without(A) over the same write must be compatible"
    );
    assert!(
        without_a.is_compatible(&with_a),
        "compatibility must be symmetric"
    );

    // and_with(A) on both: same archetypes -> not compatible.
    let mut with_a2 = FilteredAccess::default();
    with_a2.add_write(t);
    with_a2.and_with(a);
    assert!(
        !with_a.is_compatible(&with_a2),
        "two writes both narrowed to With(A) must conflict"
    );

    // No extra filters on either: both carry only the implicit with(T) -> conflict.
    let mut plain1 = FilteredAccess::default();
    plain1.add_write(t);
    let mut plain2 = FilteredAccess::default();
    plain2.add_write(t);
    assert!(
        !plain1.is_compatible(&plain2),
        "two unfiltered writes of the same component must conflict"
    );
}

#[test]
fn test_filtered_access_set_pairwise_no_cross_shielding() {
    //! INVARIANT: FilteredAccessSet compatibility is pairwise across members.
    //! A set of {write T, no filters} and {write U, and_with(A)} is INCOMPATIBLE
    //! with a set of {write T, and_without(A)}: the and_with(A) on the second
    //! member of the first set does not shield the first member's unfiltered
    //! write of T from the other set's {write T, and_without(A)} entry.
    //!
    //! Relied on by: PyBevy emits one FilteredAccess per Query/View parameter
    //! precisely because merging them into a single FilteredAccess would let one
    //! query's filter falsely narrow another query's declared access.
    //!
    //! Reevaluate if it fails: DynamicSystem::initialize / build_full_access_set
    //! could be tempted to coalesce per-parameter accesses, which would then hide
    //! genuine conflicts from the executor.

    let mut world = World::new();
    let t = world.register_component::<SchedT>();
    let u = world.register_component::<SchedU>();
    let a = world.register_component::<SchedA>();

    let mut set1 = FilteredAccessSet::default();
    let mut write_t = FilteredAccess::default();
    write_t.add_write(t);
    set1.add(write_t);
    let mut write_u_with_a = FilteredAccess::default();
    write_u_with_a.add_write(u);
    write_u_with_a.and_with(a);
    set1.add(write_u_with_a);

    let mut set2 = FilteredAccessSet::default();
    let mut write_t_without_a = FilteredAccess::default();
    write_t_without_a.add_write(t);
    write_t_without_a.and_without(a);
    set2.add(write_t_without_a);

    assert!(
        !set1.is_compatible(&set2),
        "a filtered member must not shield another member's unfiltered write"
    );
    assert!(
        !set2.is_compatible(&set1),
        "set compatibility must be symmetric"
    );
}

#[test]
fn test_changed_added_filters_declare_read_access() {
    //! INVARIANT: Changed<T> and Added<T> filters declare READ access on T in
    //! their component_access, and that access is incompatible with a
    //! Query<&mut T> that writes T.
    //!
    //! Relied on by: PyBevy implements Changed/Added via manual per-entity tick
    //! reads and must declare the same read the native filter would, so the
    //! executor serializes it against writers of T.
    //!
    //! Reevaluate if it fails: PyBevy would under-declare access for its
    //! Changed/Added emulation and could read ticks concurrently with a writer.

    let mut world = World::new();
    let t = world.register_component::<SchedT>();

    let changed = world.query_filtered::<(), Changed<SchedT>>();
    assert!(
        changed.component_access().access().has_read(t),
        "Changed<T> must declare read access on T"
    );

    let added = world.query_filtered::<(), Added<SchedT>>();
    assert!(
        added.component_access().access().has_read(t),
        "Added<T> must declare read access on T"
    );

    let writer = world.query::<&mut SchedT>();

    let mut changed_set = FilteredAccessSet::default();
    changed_set.add(changed.component_access().clone());
    let mut added_set = FilteredAccessSet::default();
    added_set.add(added.component_access().clone());
    let mut writer_set = FilteredAccessSet::default();
    writer_set.add(writer.component_access().clone());

    assert!(
        !changed_set.is_compatible(&writer_set),
        "Changed<T> read must conflict with Query<&mut T>"
    );
    assert!(
        !added_set.is_compatible(&writer_set),
        "Added<T> read must conflict with Query<&mut T>"
    );
}

#[test]
fn test_has_declares_no_access_and_no_narrowing() {
    //! INVARIANT: Query<Has<T>> matches entities that do NOT have T (Has is
    //! archetypal, not a With narrowing), and its component_access declares
    //! neither read nor write on T.
    //!
    //! Relied on by: PyBevy must not emit and_with or any read/write access for
    //! `Has[...]` filters, otherwise Has queries would wrongly skip entities
    //! missing T and would over-declare access.
    //!
    //! Reevaluate if it fails: the `Has(_)` arm in query_filters_from_query_param
    //! (which deliberately contributes no with/without) and the initialize filter
    //! handling for Has must be revisited.

    let mut world = World::new();
    let without_t = world.spawn(SchedA).id();
    let with_t = world.spawn(SchedT).id();

    let mut query = world.query::<(Entity, Has<SchedT>)>();
    let results: Vec<(Entity, bool)> = query.iter(&world).collect();

    assert!(
        results.iter().any(|&(e, has)| e == without_t && !has),
        "Has<T> must match an entity without T, yielding false"
    );
    assert!(
        results.iter().any(|&(e, has)| e == with_t && has),
        "Has<T> must yield true for an entity with T"
    );

    let t = world.register_component::<SchedT>();
    let access = query.component_access().access();
    assert!(
        !access.has_read(t),
        "Has<T> must not declare read access on T"
    );
    assert!(
        !access.has_write(t),
        "Has<T> must not declare write access on T"
    );
}

#[test]
fn test_or_filter_cannot_be_represented_conjunctively() {
    //! INVARIANT: Query<(), Or<(With<A>, With<B>)>> matches an entity that has
    //! only A (and one that has only B). An Or filter is a disjunction and
    //! cannot be flattened into a conjunction of and_with calls.
    //!
    //! Relied on by: PyBevy declares no filter narrowing for `AnyOf[...]`,
    //! because and_with(A) + and_with(B) would wrongly claim the query only
    //! touches archetypes that contain BOTH A and B.
    //!
    //! Reevaluate if it fails: the `AnyOf(_)` arm in
    //! query_filters_from_query_param (which contributes no with/without) must be
    //! reconsidered.

    let mut world = World::new();
    let only_a = world.spawn(SchedA).id();
    let only_b = world.spawn(SchedB).id();
    let both = world.spawn((SchedA, SchedB)).id();

    let mut query = world.query_filtered::<Entity, Or<(With<SchedA>, With<SchedB>)>>();
    let matched: Vec<Entity> = query.iter(&world).collect();

    assert!(
        matched.contains(&only_a),
        "Or<(With<A>, With<B>)> must match an entity with only A"
    );
    assert!(
        matched.contains(&only_b),
        "Or<(With<A>, With<B>)> must match an entity with only B"
    );
    assert!(
        matched.contains(&both),
        "Or<(With<A>, With<B>)> must match an entity with both"
    );
    assert_eq!(matched.len(), 3, "exactly the three matching entities");
}

#[test]
fn test_option_does_not_narrow_access() {
    //! INVARIANT: Option<&T> adds T's read access WITHOUT a With(T) narrowing,
    //! so the access set of Query<(&mut U, Option<&T>)> is INCOMPATIBLE with the
    //! access set of Query<&mut U, Without<T>> (both write U and the optional
    //! component does not prove archetype disjointness against Without<T>).
    //!
    //! Relied on by: PyBevy must not emit and_with for optional query
    //! components; if it did, an optional component would falsely narrow the
    //! query and appear disjoint from a Without filter over the same component.
    //!
    //! Reevaluate if it fails: PyBevy's handling of optional query components in
    //! query_filters_from_query_param (which adds no with entry for optionals)
    //! must be revisited.

    let mut world = World::new();
    world.spawn(SchedU);

    let optional = world.query::<(&mut SchedU, Option<&SchedT>)>();
    let without = world.query_filtered::<&mut SchedU, Without<SchedT>>();

    let mut optional_set = FilteredAccessSet::default();
    optional_set.add(optional.component_access().clone());
    let mut without_set = FilteredAccessSet::default();
    without_set.add(without.component_access().clone());

    assert!(
        !optional_set.is_compatible(&without_set),
        "Option<&T> must not narrow access enough to look disjoint from Without<T>"
    );
    assert!(
        !without_set.is_compatible(&optional_set),
        "set compatibility must be symmetric"
    );
}

#[test]
fn test_resources_are_components() {
    //! INVARIANT: In Bevy 0.19 resources ARE components. For a Resource type R,
    //! World::register_component::<R>() returns the same ComponentId that
    //! resource insertion/lookup uses (Components::get_id(TypeId::of::<R>())
    //! after insert_resource). Registration order does not matter; the id is
    //! keyed by TypeId in a single shared index.
    //!
    //! Relied on by: DynamicSystem::initialize declares its HotReloadGeneration
    //! resource read via world.register_component::<HotReloadGeneration>(), while
    //! run_unsafe reads the resource by type. If these diverged, the declared
    //! access would not cover the actual read.
    //!
    //! Reevaluate if it fails: resources and components would occupy separate
    //! ComponentId spaces again; initialize would need a resource-specific
    //! registration API (register_resource / init_resource) to declare the read.

    let mut world = World::new();

    let via_register = world.register_component::<SchedResource>();
    world.insert_resource(SchedResource);
    let via_lookup = world
        .components()
        .get_id(TypeId::of::<SchedResource>())
        .expect("resource TypeId must resolve to a ComponentId after insertion");

    assert_eq!(
        via_register, via_lookup,
        "register_component and resource lookup must share one ComponentId space"
    );
}

#[test]
fn test_exclusive_function_systems_declare_empty_access_and_non_send() {
    //! INVARIANT: Bevy's ExclusiveFunctionSystem (a `fn(&mut World)` system)
    //! returns an EMPTY FilteredAccessSet from initialize and its flags are
    //! NON_SEND | EXCLUSIVE. Exclusivity is enforced through the flags, not
    //! through declared access, and the NON_SEND half is load-bearing:
    //! MultiThreadedExecutor sets `local_thread_running` unconditionally when
    //! it spawns an exclusive system but clears it on completion only for
    //! non-Send systems. A Send exclusive system (unreachable in vanilla Bevy,
    //! constructible via a custom System impl) leaks the flag and permanently
    //! blocks every non-Send system still queued, ending the schedule scope on
    //! the debug assertion `state.ready_systems.is_clear()`.
    //!
    //! Relied on by: DynamicSystem's World-parameter systems return an empty
    //! access set from initialize and NON_SEND | EXCLUSIVE from flags() to
    //! match this shape; pybevy's built-in exclusive systems (e.g. the
    //! Last-schedule error drain) are plain fn(&mut World) systems.
    //!
    //! Reevaluate if it fails: Bevy changed the exclusive-system contract;
    //! DynamicSystem's World arm and compute_system_flags must mirror whatever
    //! ExclusiveFunctionSystem now declares.

    let mut world = World::new();
    let mut system = IntoSystem::into_system(|_world: &mut World| {});
    let access_set = system.initialize(&mut world);

    let combined = access_set.combined_access();
    assert!(
        !combined.has_any_read(),
        "exclusive fn systems must declare no reads"
    );
    assert!(
        !combined.has_any_write(),
        "exclusive fn systems must declare no writes"
    );
    assert!(system.is_exclusive(), "fn(&mut World) must be exclusive");
    assert!(
        !system.is_send(),
        "exclusive fn systems must be non-Send; the executor's local-thread \
         accounting assumes the pairing"
    );
}

#[test]
fn test_state_is_committed_before_transition_schedules() {
    //! BEHAVIOR: Bevy 0.19 commits the new `State<S>` value before it runs
    //! `OnExit`, `OnTransition`, and `OnEnter`. All three schedules therefore
    //! observe the entered state through `Res<State<S>>`.
    //!
    //! Relied on by: PyBevy's state-transition contract and future neutral
    //! transition planner. If this changes upstream, reevaluate the planned
    //! `CommitNew -> ExitOld -> Transition -> EnterNew` step order.

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_state::<TransitionContractState>()
        .insert_resource(TransitionContractTrace::default())
        .add_systems(OnExit(TransitionContractState::Alpha), record_contract_exit)
        .add_systems(
            OnTransition {
                exited: TransitionContractState::Alpha,
                entered: TransitionContractState::Beta,
            },
            record_contract_transition,
        )
        .add_systems(
            OnEnter(TransitionContractState::Beta),
            record_contract_enter,
        );

    // Drain the initial Alpha OnEnter event before testing an ordinary change.
    app.update();
    app.world_mut()
        .resource_mut::<TransitionContractTrace>()
        .0
        .clear();
    app.world_mut()
        .resource_mut::<NextState<TransitionContractState>>()
        .set(TransitionContractState::Beta);

    app.update();

    assert_eq!(
        app.world().resource::<TransitionContractTrace>().0,
        [
            ("exit", TransitionContractState::Beta),
            ("transition", TransitionContractState::Beta),
            ("enter", TransitionContractState::Beta),
        ]
    );
}

#[test]
fn test_pending_transition_before_first_update_supersedes_initial_enter() {
    //! BEHAVIOR: If a new state is already pending before the first
    //! `StateTransition` schedule, Bevy consumes the latest transition event.
    //! It does not run the stale initial state's `OnEnter` first or replay the
    //! entered state's `OnEnter` on the next update.

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_state::<TransitionContractState>()
        .insert_resource(TransitionContractTrace::default())
        .add_systems(
            OnEnter(TransitionContractState::Alpha),
            record_contract_initial_enter,
        )
        .add_systems(OnExit(TransitionContractState::Alpha), record_contract_exit)
        .add_systems(
            OnTransition {
                exited: TransitionContractState::Alpha,
                entered: TransitionContractState::Beta,
            },
            record_contract_transition,
        )
        .add_systems(
            OnEnter(TransitionContractState::Beta),
            record_contract_enter,
        );
    app.world_mut()
        .resource_mut::<NextState<TransitionContractState>>()
        .set(TransitionContractState::Beta);

    app.update();
    app.update();

    assert_eq!(
        app.world().resource::<TransitionContractTrace>().0,
        [
            ("exit", TransitionContractState::Beta),
            ("transition", TransitionContractState::Beta),
            ("enter", TransitionContractState::Beta),
        ]
    );
}
