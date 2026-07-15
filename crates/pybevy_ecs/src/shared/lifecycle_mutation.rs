//! Interpreter-neutral lifecycle mutation ordering.
//!
//! This module deliberately owns no Python values, Bevy `World` borrow, or
//! observer registry guard. An adapter performs those backend-specific jobs and
//! must return from each method without retaining a borrow into its registry or
//! world. Observer invocation may synchronously re-enter the adapter and
//! structurally mutate the target, so the core rechecks liveness and component
//! membership at every callback boundary. Matching observer handles are
//! snapshotted into an owned vector before the core invokes them.

/// Lifecycle callback selected by the mutation planner.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LifecycleEvent {
    Add,
    Insert,
    Remove,
    Discard,
    Despawn,
}

/// Why a planned mutation stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleMutationOutcome {
    /// The structural operation and every applicable post-event completed.
    Complete,
    /// A callback removed the entity before the next planned step.
    EntityGone,
    /// A callback or structural hook removed the component before a required
    /// component-specific post-event.
    ComponentGone,
}

/// Backend operations required by [`LifecycleMutationCore`].
///
/// Implementations must not return guards or references tied to the World or
/// observer registry. Each callback gets a fresh command queue; run-scoped
/// parameters are invalidated before that queue is applied, and callback errors
/// are reported without preventing later still-applicable callbacks. Structural
/// methods may trigger native Bevy hooks and are therefore treated as reentrant
/// callback boundaries too.
pub trait LifecycleMutationAdapter {
    type Entity: Copy;
    type Component: Copy;
    type Observer;

    fn entity_exists(&self, entity: Self::Entity) -> bool;

    fn component_exists(&self, entity: Self::Entity, component: Self::Component) -> bool;

    /// Snapshot matching prepared observer handles in registration order.
    /// The returned values must own everything needed for an in-flight call so
    /// this method retains no registry or World borrow.
    fn snapshot_observers(
        &mut self,
        event: LifecycleEvent,
        entity: Self::Entity,
        component: Self::Component,
    ) -> Vec<Self::Observer>;

    /// Invoke exactly one prepared observer.
    ///
    /// The core performs the live entity/component admission check immediately
    /// before this call. The adapter owns backend-specific argument creation,
    /// validity invalidation, command application, and off-World error reporting.
    fn invoke_observer(
        &mut self,
        observer: &Self::Observer,
        event: LifecycleEvent,
        entity: Self::Entity,
        component: Self::Component,
    );

    fn insert_component(&mut self, entity: Self::Entity, component: Self::Component);

    fn remove_component(&mut self, entity: Self::Entity, component: Self::Component);

    fn despawn_entity(&mut self, entity: Self::Entity);

    /// Remove target and observer reverse-index entries after every Despawn
    /// callback has completed, but before structural despawn.
    ///
    /// This operation must be idempotent: callbacks and cleanup finalizers may
    /// already have despawned the target, and recursive despawn snapshots may
    /// ask for cleanup of a member that no longer exists.
    fn cleanup_despawned_entity(&mut self, entity: Self::Entity);
}

/// One entity and its duplicate-free Python-visible component identities,
/// captured before a recursive despawn begins dispatching callbacks.
///
/// The snapshot deliberately owns the component list. Components inserted on
/// this entity by an earlier callback are therefore excluded from the in-flight
/// recursive despawn's lifecycle passes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecursiveDespawnSnapshot<E, C> {
    pub entity: E,
    pub components: Vec<C>,
}

impl<E, C> RecursiveDespawnSnapshot<E, C> {
    pub fn new(entity: E, components: Vec<C>) -> Self {
        Self { entity, components }
    }
}

/// Orders lifecycle callbacks around structural mutations.
///
/// This first slice is exercised only with fake adapters. Binding adapters are
/// intentionally not migrated until the ordering and reentrancy contract has
/// received architectural review.
#[derive(Clone, Copy, Debug, Default)]
pub struct LifecycleMutationCore;

impl LifecycleMutationCore {
    /// Insert or replace one component.
    ///
    /// Presence is sampled once to decide whether `Discard` applies, then
    /// sampled again after `Discard` to decide whether the eventual insertion
    /// is an `Add`. This prevents a callback from leaving a stale pre-dispatch
    /// classification behind.
    pub fn insert<A: LifecycleMutationAdapter>(
        &self,
        adapter: &mut A,
        entity: A::Entity,
        component: A::Component,
    ) -> LifecycleMutationOutcome {
        if !adapter.entity_exists(entity) {
            return LifecycleMutationOutcome::EntityGone;
        }

        if adapter.component_exists(entity, component) {
            Self::dispatch_event(adapter, LifecycleEvent::Discard, entity, component);
        }

        if !adapter.entity_exists(entity) {
            return LifecycleMutationOutcome::EntityGone;
        }
        let is_add = !adapter.component_exists(entity, component);

        adapter.insert_component(entity, component);
        if !adapter.entity_exists(entity) {
            return LifecycleMutationOutcome::EntityGone;
        }
        if !adapter.component_exists(entity, component) {
            return LifecycleMutationOutcome::ComponentGone;
        }

        if is_add {
            Self::dispatch_event(adapter, LifecycleEvent::Add, entity, component);
            if !adapter.entity_exists(entity) {
                return LifecycleMutationOutcome::EntityGone;
            }
            if !adapter.component_exists(entity, component) {
                return LifecycleMutationOutcome::ComponentGone;
            }
        }

        Self::dispatch_event(adapter, LifecycleEvent::Insert, entity, component);
        LifecycleMutationOutcome::Complete
    }

    /// Remove one component, dispatching `Discard` and then `Remove` while its
    /// old value remains readable. A callback may satisfy the removal itself;
    /// the outer removal then becomes a no-op.
    pub fn remove<A: LifecycleMutationAdapter>(
        &self,
        adapter: &mut A,
        entity: A::Entity,
        component: A::Component,
    ) -> LifecycleMutationOutcome {
        if !adapter.entity_exists(entity) {
            return LifecycleMutationOutcome::EntityGone;
        }
        if !adapter.component_exists(entity, component) {
            return LifecycleMutationOutcome::ComponentGone;
        }

        Self::dispatch_event(adapter, LifecycleEvent::Discard, entity, component);
        if !adapter.entity_exists(entity) {
            return LifecycleMutationOutcome::EntityGone;
        }
        if !adapter.component_exists(entity, component) {
            return LifecycleMutationOutcome::Complete;
        }

        Self::dispatch_event(adapter, LifecycleEvent::Remove, entity, component);
        if !adapter.entity_exists(entity) {
            return LifecycleMutationOutcome::EntityGone;
        }
        if !adapter.component_exists(entity, component) {
            return LifecycleMutationOutcome::Complete;
        }

        adapter.remove_component(entity, component);

        LifecycleMutationOutcome::Complete
    }

    /// Despawn an entity after `Despawn`, `Discard`, and `Remove` passes over
    /// its initial component snapshot.
    ///
    /// Component identities are supplied by the backend because Python type
    /// ownership and `ComponentId` ↔ interpreter-type mapping remain adapter
    /// concerns. The slice must contain each component identity at most once.
    pub fn despawn<A: LifecycleMutationAdapter>(
        &self,
        adapter: &mut A,
        entity: A::Entity,
        components: &[A::Component],
    ) -> LifecycleMutationOutcome {
        let entity_gone = !Self::dispatch_despawn_passes(adapter, entity, components);
        adapter.cleanup_despawned_entity(entity);

        if entity_gone || !adapter.entity_exists(entity) {
            return LifecycleMutationOutcome::EntityGone;
        }

        adapter.despawn_entity(entity);
        LifecycleMutationOutcome::Complete
    }

    /// Execute a recursive despawn without allowing the root's native cascade
    /// to delete descendants before their Python lifecycle callbacks run.
    ///
    /// `snapshots` must contain the root first, followed by descendants in the
    /// adapter's chosen stable traversal order. Every component list must be
    /// duplicate-free and captured before any callback runs.
    pub fn despawn_recursive<A: LifecycleMutationAdapter>(
        &self,
        adapter: &mut A,
        snapshots: &[RecursiveDespawnSnapshot<A::Entity, A::Component>],
    ) -> LifecycleMutationOutcome {
        let Some(root) = snapshots.first().map(|snapshot| snapshot.entity) else {
            return LifecycleMutationOutcome::Complete;
        };
        if !adapter.entity_exists(root) {
            for snapshot in snapshots {
                adapter.cleanup_despawned_entity(snapshot.entity);
            }
            return LifecycleMutationOutcome::EntityGone;
        }

        // Dispatch and cleanup are a separate phase from structural deletion:
        // despawning the root through Bevy may synchronously cascade through
        // every currently related descendant.
        for snapshot in snapshots {
            Self::dispatch_despawn_passes(adapter, snapshot.entity, &snapshot.components);
            adapter.cleanup_despawned_entity(snapshot.entity);
        }

        let root_survived_dispatch = adapter.entity_exists(root);
        if root_survived_dispatch {
            adapter.despawn_entity(root);
        }

        // A callback may have reparented a snapshotted descendant out of the
        // root's live relationship tree. The initial set remains authoritative,
        // so delete any such survivor explicitly.
        for snapshot in snapshots {
            if adapter.entity_exists(snapshot.entity) {
                adapter.despawn_entity(snapshot.entity);
            }
        }

        if root_survived_dispatch {
            LifecycleMutationOutcome::Complete
        } else {
            LifecycleMutationOutcome::EntityGone
        }
    }

    fn dispatch_despawn_passes<A: LifecycleMutationAdapter>(
        adapter: &mut A,
        entity: A::Entity,
        components: &[A::Component],
    ) -> bool {
        for event in [
            LifecycleEvent::Despawn,
            LifecycleEvent::Discard,
            LifecycleEvent::Remove,
        ] {
            for component in components {
                if !adapter.entity_exists(entity) {
                    return false;
                }
                if !adapter.component_exists(entity, *component) {
                    continue;
                }

                Self::dispatch_event(adapter, event, entity, *component);
            }
        }

        adapter.entity_exists(entity)
    }

    fn dispatch_event<A: LifecycleMutationAdapter>(
        adapter: &mut A,
        event: LifecycleEvent,
        entity: A::Entity,
        component: A::Component,
    ) {
        let observers = adapter.snapshot_observers(event, entity, component);
        for observer in &observers {
            if !adapter.entity_exists(entity) || !adapter.component_exists(entity, component) {
                break;
            }
            adapter.invoke_observer(observer, event, entity, component);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    use super::{
        LifecycleEvent, LifecycleMutationAdapter, LifecycleMutationCore, LifecycleMutationOutcome,
        RecursiveDespawnSnapshot,
    };

    type Entity = u32;
    type Component = u32;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum CallbackAction {
        Noop,
        RemoveComponent,
        RemoveOtherComponent(Component),
        InsertOtherComponent(Component),
        DespawnEntity,
        ReinsertComponent,
        ClearRegistrations,
        RegisterObserver,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Step {
        Dispatch {
            event: LifecycleEvent,
            component: Component,
            readable: bool,
        },
        Insert(Component),
        Remove(Component),
        Despawn,
        Cleanup,
    }

    struct FakeAdapter {
        entity: Entity,
        alive: bool,
        components: HashSet<Component>,
        actions: HashMap<(LifecycleEvent, Component), Vec<CallbackAction>>,
        cleanup_despawns: bool,
        steps: Vec<Step>,
    }

    impl FakeAdapter {
        fn new(entity: Entity, components: impl IntoIterator<Item = Component>) -> Self {
            Self {
                entity,
                alive: true,
                components: components.into_iter().collect(),
                actions: HashMap::new(),
                cleanup_despawns: false,
                steps: Vec::new(),
            }
        }

        fn on(
            mut self,
            event: LifecycleEvent,
            component: Component,
            action: CallbackAction,
        ) -> Self {
            self.actions
                .entry((event, component))
                .or_default()
                .push(action);
            self
        }

        fn despawn_during_cleanup(mut self) -> Self {
            self.cleanup_despawns = true;
            self
        }
    }

    impl LifecycleMutationAdapter for FakeAdapter {
        type Entity = Entity;
        type Component = Component;
        type Observer = CallbackAction;

        fn entity_exists(&self, entity: Entity) -> bool {
            entity == self.entity && self.alive
        }

        fn component_exists(&self, entity: Entity, component: Component) -> bool {
            self.entity_exists(entity) && self.components.contains(&component)
        }

        fn snapshot_observers(
            &mut self,
            event: LifecycleEvent,
            _entity: Entity,
            component: Component,
        ) -> Vec<Self::Observer> {
            self.actions
                .get(&(event, component))
                .cloned()
                .unwrap_or_else(|| vec![CallbackAction::Noop])
        }

        fn invoke_observer(
            &mut self,
            observer: &Self::Observer,
            event: LifecycleEvent,
            entity: Entity,
            component: Component,
        ) {
            self.steps.push(Step::Dispatch {
                event,
                component,
                readable: self.component_exists(entity, component),
            });
            match *observer {
                CallbackAction::Noop => {}
                CallbackAction::RemoveComponent => {
                    self.components.remove(&component);
                }
                CallbackAction::RemoveOtherComponent(other) => {
                    self.components.remove(&other);
                }
                CallbackAction::InsertOtherComponent(other) => {
                    self.components.insert(other);
                }
                CallbackAction::DespawnEntity => {
                    self.alive = false;
                    self.components.clear();
                }
                CallbackAction::ReinsertComponent => {
                    self.components.insert(component);
                }
                CallbackAction::ClearRegistrations => {
                    self.actions.remove(&(event, component));
                }
                CallbackAction::RegisterObserver => {
                    self.actions
                        .entry((event, component))
                        .or_default()
                        .push(CallbackAction::Noop);
                }
            }
        }

        fn insert_component(&mut self, entity: Entity, component: Component) {
            assert!(self.entity_exists(entity));
            self.steps.push(Step::Insert(component));
            self.components.insert(component);
        }

        fn remove_component(&mut self, entity: Entity, component: Component) {
            assert!(self.entity_exists(entity));
            self.steps.push(Step::Remove(component));
            self.components.remove(&component);
        }

        fn despawn_entity(&mut self, entity: Entity) {
            assert!(self.entity_exists(entity));
            self.steps.push(Step::Despawn);
            self.alive = false;
            self.components.clear();
        }

        fn cleanup_despawned_entity(&mut self, entity: Entity) {
            assert_eq!(entity, self.entity);
            self.steps.push(Step::Cleanup);
            if self.cleanup_despawns {
                self.alive = false;
                self.components.clear();
            }
        }
    }

    #[test]
    fn first_insert_dispatches_add_then_insert_after_storage_exists() {
        let mut adapter = FakeAdapter::new(7, []);

        let outcome = LifecycleMutationCore.insert(&mut adapter, 7, 10);

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert_eq!(
            adapter.steps,
            [
                Step::Insert(10),
                Step::Dispatch {
                    event: LifecycleEvent::Add,
                    component: 10,
                    readable: true,
                },
                Step::Dispatch {
                    event: LifecycleEvent::Insert,
                    component: 10,
                    readable: true,
                },
            ]
        );
    }

    #[test]
    fn replacement_dispatches_discard_before_overwrite_without_add() {
        let mut adapter = FakeAdapter::new(7, [10]);

        let outcome = LifecycleMutationCore.insert(&mut adapter, 7, 10);

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert_eq!(
            adapter.steps,
            [
                Step::Dispatch {
                    event: LifecycleEvent::Discard,
                    component: 10,
                    readable: true,
                },
                Step::Insert(10),
                Step::Dispatch {
                    event: LifecycleEvent::Insert,
                    component: 10,
                    readable: true,
                },
            ]
        );
    }

    #[test]
    fn discard_removal_reclassifies_replacement_as_add() {
        let mut adapter = FakeAdapter::new(7, [10]).on(
            LifecycleEvent::Discard,
            10,
            CallbackAction::RemoveComponent,
        );

        let outcome = LifecycleMutationCore.insert(&mut adapter, 7, 10);

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert_eq!(
            adapter.steps,
            [
                Step::Dispatch {
                    event: LifecycleEvent::Discard,
                    component: 10,
                    readable: true,
                },
                Step::Insert(10),
                Step::Dispatch {
                    event: LifecycleEvent::Add,
                    component: 10,
                    readable: true,
                },
                Step::Dispatch {
                    event: LifecycleEvent::Insert,
                    component: 10,
                    readable: true,
                },
            ]
        );
    }

    #[test]
    fn discard_despawn_stops_before_structural_insert() {
        let mut adapter = FakeAdapter::new(7, [10]).on(
            LifecycleEvent::Discard,
            10,
            CallbackAction::DespawnEntity,
        );

        let outcome = LifecycleMutationCore.insert(&mut adapter, 7, 10);

        assert_eq!(outcome, LifecycleMutationOutcome::EntityGone);
        assert_eq!(
            adapter.steps,
            [Step::Dispatch {
                event: LifecycleEvent::Discard,
                component: 10,
                readable: true,
            }]
        );
    }

    #[test]
    fn add_removal_suppresses_stale_insert_callback() {
        let mut adapter =
            FakeAdapter::new(7, []).on(LifecycleEvent::Add, 10, CallbackAction::RemoveComponent);

        let outcome = LifecycleMutationCore.insert(&mut adapter, 7, 10);

        assert_eq!(outcome, LifecycleMutationOutcome::ComponentGone);
        assert_eq!(
            adapter.steps,
            [
                Step::Insert(10),
                Step::Dispatch {
                    event: LifecycleEvent::Add,
                    component: 10,
                    readable: true,
                },
            ]
        );
    }

    #[test]
    fn remove_dispatches_discard_then_remove_while_value_is_readable() {
        let mut adapter = FakeAdapter::new(7, [10]);

        let outcome = LifecycleMutationCore.remove(&mut adapter, 7, 10);

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert_eq!(
            adapter.steps,
            [
                Step::Dispatch {
                    event: LifecycleEvent::Discard,
                    component: 10,
                    readable: true,
                },
                Step::Dispatch {
                    event: LifecycleEvent::Remove,
                    component: 10,
                    readable: true,
                },
                Step::Remove(10),
            ]
        );
    }

    #[test]
    fn discard_callback_can_complete_remove_without_duplicate_remove_event() {
        let mut adapter = FakeAdapter::new(7, [10]).on(
            LifecycleEvent::Discard,
            10,
            CallbackAction::RemoveComponent,
        );

        let outcome = LifecycleMutationCore.remove(&mut adapter, 7, 10);

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert_eq!(
            adapter.steps,
            [Step::Dispatch {
                event: LifecycleEvent::Discard,
                component: 10,
                readable: true,
            }]
        );
    }

    #[test]
    fn despawn_dispatches_event_major_passes_then_cleanup_then_mutation() {
        let mut adapter = FakeAdapter::new(7, [10, 20]);

        let outcome = LifecycleMutationCore.despawn(&mut adapter, 7, &[10, 20]);

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert_eq!(
            adapter.steps,
            [
                Step::Dispatch {
                    event: LifecycleEvent::Despawn,
                    component: 10,
                    readable: true,
                },
                Step::Dispatch {
                    event: LifecycleEvent::Despawn,
                    component: 20,
                    readable: true,
                },
                Step::Dispatch {
                    event: LifecycleEvent::Discard,
                    component: 10,
                    readable: true,
                },
                Step::Dispatch {
                    event: LifecycleEvent::Discard,
                    component: 20,
                    readable: true,
                },
                Step::Dispatch {
                    event: LifecycleEvent::Remove,
                    component: 10,
                    readable: true,
                },
                Step::Dispatch {
                    event: LifecycleEvent::Remove,
                    component: 20,
                    readable: true,
                },
                Step::Cleanup,
                Step::Despawn,
            ]
        );
    }

    #[test]
    fn despawn_cleanup_runs_when_callback_already_removed_target() {
        let mut adapter = FakeAdapter::new(7, [10]).on(
            LifecycleEvent::Despawn,
            10,
            CallbackAction::DespawnEntity,
        );

        let outcome = LifecycleMutationCore.despawn(&mut adapter, 7, &[10]);

        assert_eq!(outcome, LifecycleMutationOutcome::EntityGone);
        assert_eq!(
            adapter.steps,
            [
                Step::Dispatch {
                    event: LifecycleEvent::Despawn,
                    component: 10,
                    readable: true,
                },
                Step::Cleanup,
            ]
        );
    }

    #[test]
    fn despawn_cleanup_runs_when_target_is_already_gone() {
        let mut adapter = FakeAdapter::new(7, [10]);
        adapter.alive = false;
        adapter.components.clear();

        let outcome = LifecycleMutationCore.despawn(&mut adapter, 7, &[10]);

        assert_eq!(outcome, LifecycleMutationOutcome::EntityGone);
        assert_eq!(adapter.steps, [Step::Cleanup]);
    }

    #[test]
    fn cleanup_reentrancy_suppresses_second_structural_despawn() {
        let mut adapter = FakeAdapter::new(7, [10]).despawn_during_cleanup();

        let outcome = LifecycleMutationCore.despawn(&mut adapter, 7, &[10]);

        assert_eq!(outcome, LifecycleMutationOutcome::EntityGone);
        assert!(matches!(adapter.steps.last(), Some(Step::Cleanup)));
        assert!(!adapter.steps.contains(&Step::Despawn));
    }

    #[test]
    fn despawn_rechecks_each_component_between_callbacks_and_passes() {
        let mut adapter = FakeAdapter::new(7, [10, 20]).on(
            LifecycleEvent::Despawn,
            10,
            CallbackAction::RemoveOtherComponent(20),
        );

        let outcome = LifecycleMutationCore.despawn(&mut adapter, 7, &[10, 20]);

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert!(
            !adapter
                .steps
                .iter()
                .any(|step| { matches!(step, Step::Dispatch { component: 20, .. }) })
        );
    }

    #[test]
    fn despawn_snapshot_does_not_grow_when_callback_inserts_component() {
        let mut adapter = FakeAdapter::new(7, [10]).on(
            LifecycleEvent::Despawn,
            10,
            CallbackAction::InsertOtherComponent(20),
        );

        let outcome = LifecycleMutationCore.despawn(&mut adapter, 7, &[10]);

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert!(
            !adapter
                .steps
                .iter()
                .any(|step| { matches!(step, Step::Dispatch { component: 20, .. }) })
        );
    }

    #[test]
    fn discard_reinsert_keeps_replacement_classification() {
        let mut adapter = FakeAdapter::new(7, [10]).on(
            LifecycleEvent::Discard,
            10,
            CallbackAction::ReinsertComponent,
        );

        let outcome = LifecycleMutationCore.insert(&mut adapter, 7, 10);

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert!(!adapter.steps.iter().any(|step| {
            matches!(
                step,
                Step::Dispatch {
                    event: LifecycleEvent::Add,
                    ..
                }
            )
        }));
    }

    struct DropProbe(Arc<AtomicBool>);

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    struct PanickingAdapter {
        observer_dropped: Arc<AtomicBool>,
    }

    impl LifecycleMutationAdapter for PanickingAdapter {
        type Entity = Entity;
        type Component = Component;
        type Observer = DropProbe;

        fn entity_exists(&self, _entity: Entity) -> bool {
            true
        }

        fn component_exists(&self, _entity: Entity, _component: Component) -> bool {
            true
        }

        fn snapshot_observers(
            &mut self,
            _event: LifecycleEvent,
            _entity: Entity,
            _component: Component,
        ) -> Vec<Self::Observer> {
            vec![DropProbe(self.observer_dropped.clone())]
        }

        fn invoke_observer(
            &mut self,
            _observer: &Self::Observer,
            _event: LifecycleEvent,
            _entity: Entity,
            _component: Component,
        ) {
            panic!("fake observer panic");
        }

        fn insert_component(&mut self, _entity: Entity, _component: Component) {}

        fn remove_component(&mut self, _entity: Entity, _component: Component) {}

        fn despawn_entity(&mut self, _entity: Entity) {}

        fn cleanup_despawned_entity(&mut self, _entity: Entity) {}
    }

    #[test]
    fn observer_snapshot_handles_drop_during_panic_unwind() {
        let observer_dropped = Arc::new(AtomicBool::new(false));
        let mut adapter = PanickingAdapter {
            observer_dropped: observer_dropped.clone(),
        };

        let result = catch_unwind(AssertUnwindSafe(|| {
            LifecycleMutationCore.despawn(&mut adapter, 7, &[10]);
        }));

        assert!(result.is_err());
        assert!(observer_dropped.load(Ordering::Acquire));
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum RecursiveAction {
        Noop,
        ReparentOut(Entity),
        AttachChild(Entity),
        InsertComponent {
            entity: Entity,
            component: Component,
        },
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum RecursiveStep {
        Dispatch {
            event: LifecycleEvent,
            entity: Entity,
            component: Component,
        },
        Cleanup(Entity),
        Despawn(Entity),
    }

    struct RecursiveFakeAdapter {
        alive: HashSet<Entity>,
        components: HashMap<Entity, HashSet<Component>>,
        children: HashMap<Entity, Vec<Entity>>,
        actions: HashMap<(LifecycleEvent, Entity, Component), Vec<RecursiveAction>>,
        steps: Vec<RecursiveStep>,
    }

    impl RecursiveFakeAdapter {
        fn new(
            entities: impl IntoIterator<Item = (Entity, Vec<Component>)>,
            relationships: impl IntoIterator<Item = (Entity, Vec<Entity>)>,
        ) -> Self {
            let components = entities
                .into_iter()
                .map(|(entity, components)| (entity, components.into_iter().collect()))
                .collect::<HashMap<_, _>>();
            Self {
                alive: components.keys().copied().collect(),
                components,
                children: relationships.into_iter().collect(),
                actions: HashMap::new(),
                steps: Vec::new(),
            }
        }

        fn on(
            mut self,
            event: LifecycleEvent,
            entity: Entity,
            component: Component,
            action: RecursiveAction,
        ) -> Self {
            self.actions
                .entry((event, entity, component))
                .or_default()
                .push(action);
            self
        }

        fn remove_from_relationships(&mut self, entity: Entity) {
            for children in self.children.values_mut() {
                children.retain(|child| *child != entity);
            }
        }

        fn despawn_subtree(&mut self, entity: Entity) {
            let children = self.children.remove(&entity).unwrap_or_default();
            for child in children {
                self.despawn_subtree(child);
            }
            self.remove_from_relationships(entity);
            self.components.remove(&entity);
            self.alive.remove(&entity);
        }
    }

    impl LifecycleMutationAdapter for RecursiveFakeAdapter {
        type Entity = Entity;
        type Component = Component;
        type Observer = RecursiveAction;

        fn entity_exists(&self, entity: Entity) -> bool {
            self.alive.contains(&entity)
        }

        fn component_exists(&self, entity: Entity, component: Component) -> bool {
            self.components
                .get(&entity)
                .is_some_and(|components| components.contains(&component))
        }

        fn snapshot_observers(
            &mut self,
            event: LifecycleEvent,
            entity: Entity,
            component: Component,
        ) -> Vec<Self::Observer> {
            self.actions
                .get(&(event, entity, component))
                .cloned()
                .unwrap_or_else(|| vec![RecursiveAction::Noop])
        }

        fn invoke_observer(
            &mut self,
            observer: &Self::Observer,
            event: LifecycleEvent,
            entity: Entity,
            component: Component,
        ) {
            assert!(self.component_exists(entity, component));
            self.steps.push(RecursiveStep::Dispatch {
                event,
                entity,
                component,
            });

            match *observer {
                RecursiveAction::Noop => {}
                RecursiveAction::ReparentOut(member) => {
                    self.remove_from_relationships(member);
                }
                RecursiveAction::AttachChild(child) => {
                    self.remove_from_relationships(child);
                    self.children.entry(entity).or_default().push(child);
                }
                RecursiveAction::InsertComponent { entity, component } => {
                    self.components.entry(entity).or_default().insert(component);
                }
            }
        }

        fn insert_component(&mut self, entity: Entity, component: Component) {
            self.components.entry(entity).or_default().insert(component);
        }

        fn remove_component(&mut self, entity: Entity, component: Component) {
            if let Some(components) = self.components.get_mut(&entity) {
                components.remove(&component);
            }
        }

        fn despawn_entity(&mut self, entity: Entity) {
            assert!(self.entity_exists(entity));
            self.steps.push(RecursiveStep::Despawn(entity));
            self.despawn_subtree(entity);
        }

        fn cleanup_despawned_entity(&mut self, entity: Entity) {
            self.steps.push(RecursiveStep::Cleanup(entity));
        }
    }

    fn recursive_snapshots() -> Vec<RecursiveDespawnSnapshot<Entity, Component>> {
        vec![
            RecursiveDespawnSnapshot::new(1, vec![10]),
            RecursiveDespawnSnapshot::new(2, vec![20]),
        ]
    }

    #[test]
    fn recursive_despawn_dispatches_and_cleans_every_member_before_root_cascade() {
        let mut adapter = RecursiveFakeAdapter::new([(1, vec![10]), (2, vec![20])], [(1, vec![2])]);

        let outcome = LifecycleMutationCore.despawn_recursive(&mut adapter, &recursive_snapshots());

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        let first_despawn = adapter
            .steps
            .iter()
            .position(|step| matches!(step, RecursiveStep::Despawn(_)))
            .unwrap();
        assert!(adapter.steps[..first_despawn].contains(&RecursiveStep::Cleanup(1)));
        assert!(adapter.steps[..first_despawn].contains(&RecursiveStep::Cleanup(2)));
        assert!(adapter.steps[..first_despawn].iter().any(|step| {
            matches!(
                step,
                RecursiveStep::Dispatch {
                    entity: 2,
                    event: LifecycleEvent::Remove,
                    component: 20,
                }
            )
        }));
        assert_eq!(adapter.steps[first_despawn], RecursiveStep::Despawn(1));
        assert!(!adapter.entity_exists(1));
        assert!(!adapter.entity_exists(2));
    }

    #[test]
    fn recursive_despawn_deletes_reparented_snapshot_member_after_root() {
        let mut adapter = RecursiveFakeAdapter::new([(1, vec![10]), (2, vec![20])], [(1, vec![2])])
            .on(
                LifecycleEvent::Despawn,
                1,
                10,
                RecursiveAction::ReparentOut(2),
            );

        let outcome = LifecycleMutationCore.despawn_recursive(&mut adapter, &recursive_snapshots());

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert!(
            adapter
                .steps
                .ends_with(&[RecursiveStep::Despawn(1), RecursiveStep::Despawn(2),])
        );
        assert!(!adapter.entity_exists(2));
    }

    #[test]
    fn recursive_despawn_cascades_newcomer_without_dispatch() {
        let mut adapter = RecursiveFakeAdapter::new(
            [(1, vec![10]), (2, vec![20]), (3, vec![30])],
            [(1, vec![2])],
        )
        .on(
            LifecycleEvent::Despawn,
            1,
            10,
            RecursiveAction::AttachChild(3),
        );

        let outcome = LifecycleMutationCore.despawn_recursive(&mut adapter, &recursive_snapshots());

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert!(!adapter.entity_exists(3));
        assert!(
            !adapter
                .steps
                .iter()
                .any(|step| { matches!(step, RecursiveStep::Dispatch { entity: 3, .. }) })
        );
    }

    #[test]
    fn recursive_component_snapshot_excludes_later_descendant_insert() {
        let mut adapter = RecursiveFakeAdapter::new([(1, vec![10]), (2, vec![20])], [(1, vec![2])])
            .on(
                LifecycleEvent::Despawn,
                1,
                10,
                RecursiveAction::InsertComponent {
                    entity: 2,
                    component: 30,
                },
            );

        let outcome = LifecycleMutationCore.despawn_recursive(&mut adapter, &recursive_snapshots());

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert!(!adapter.steps.iter().any(|step| {
            matches!(
                step,
                RecursiveStep::Dispatch {
                    entity: 2,
                    component: 30,
                    ..
                }
            )
        }));
    }

    #[test]
    fn dispatch_rechecks_component_between_prepared_observers() {
        let mut adapter = FakeAdapter::new(7, [10])
            .on(LifecycleEvent::Despawn, 10, CallbackAction::RemoveComponent)
            .on(LifecycleEvent::Despawn, 10, CallbackAction::Noop);

        let outcome = LifecycleMutationCore.despawn(&mut adapter, 7, &[10]);

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert_eq!(
            adapter
                .steps
                .iter()
                .filter(|step| matches!(step, Step::Dispatch { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn unregistering_does_not_cancel_an_inflight_snapshot() {
        let mut adapter = FakeAdapter::new(7, [10])
            .on(
                LifecycleEvent::Despawn,
                10,
                CallbackAction::ClearRegistrations,
            )
            .on(LifecycleEvent::Despawn, 10, CallbackAction::Noop);

        let outcome = LifecycleMutationCore.despawn(&mut adapter, 7, &[10]);

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert_eq!(
            adapter
                .steps
                .iter()
                .filter(|step| {
                    matches!(
                        step,
                        Step::Dispatch {
                            event: LifecycleEvent::Despawn,
                            ..
                        }
                    )
                })
                .count(),
            2
        );
    }

    #[test]
    fn observer_registered_during_dispatch_waits_for_next_snapshot() {
        let mut adapter = FakeAdapter::new(7, [10]).on(
            LifecycleEvent::Discard,
            10,
            CallbackAction::RegisterObserver,
        );

        assert_eq!(
            LifecycleMutationCore.insert(&mut adapter, 7, 10),
            LifecycleMutationOutcome::Complete
        );
        let first_dispatch_count = adapter
            .steps
            .iter()
            .filter(|step| {
                matches!(
                    step,
                    Step::Dispatch {
                        event: LifecycleEvent::Discard,
                        ..
                    }
                )
            })
            .count();

        assert_eq!(
            LifecycleMutationCore.insert(&mut adapter, 7, 10),
            LifecycleMutationOutcome::Complete
        );
        let total_dispatch_count = adapter
            .steps
            .iter()
            .filter(|step| {
                matches!(
                    step,
                    Step::Dispatch {
                        event: LifecycleEvent::Discard,
                        ..
                    }
                )
            })
            .count();

        assert_eq!(first_dispatch_count, 1);
        assert_eq!(total_dispatch_count - first_dispatch_count, 2);
    }

    #[test]
    fn recursive_despawn_stale_root_only_cleans_snapshotted_members() {
        let mut adapter = RecursiveFakeAdapter::new([(1, vec![10]), (2, vec![20])], [(1, vec![2])]);
        adapter.despawn_subtree(1);
        adapter.alive.insert(2);
        adapter.components.insert(2, HashSet::from([20]));

        let outcome = LifecycleMutationCore.despawn_recursive(&mut adapter, &recursive_snapshots());

        assert_eq!(outcome, LifecycleMutationOutcome::EntityGone);
        assert_eq!(
            adapter.steps,
            [RecursiveStep::Cleanup(1), RecursiveStep::Cleanup(2)]
        );
        assert!(adapter.entity_exists(2));
    }
}
