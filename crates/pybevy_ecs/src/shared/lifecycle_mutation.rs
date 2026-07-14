//! Interpreter-neutral lifecycle mutation ordering.
//!
//! This module deliberately owns no Python values, Bevy `World` borrow, or
//! observer registry guard. An adapter performs those backend-specific jobs and
//! must return from each method without retaining a borrow into its registry or
//! world. `dispatch` may synchronously re-enter the adapter and structurally
//! mutate the target, so the core rechecks liveness and component membership at
//! every callback boundary.

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
/// observer registry. `dispatch` must snapshot callback handles before invoking
/// them, and must release registry borrows before Python or other user code can
/// run. Structural methods may trigger native Bevy hooks and are therefore
/// treated as reentrant callback boundaries too.
pub trait LifecycleMutationAdapter {
    type Entity: Copy;
    type Component: Copy;

    fn entity_exists(&self, entity: Self::Entity) -> bool;

    fn component_exists(&self, entity: Self::Entity, component: Self::Component) -> bool;

    fn dispatch(&mut self, event: LifecycleEvent, entity: Self::Entity, component: Self::Component);

    fn insert_component(&mut self, entity: Self::Entity, component: Self::Component);

    fn remove_component(&mut self, entity: Self::Entity, component: Self::Component);

    fn despawn_entity(&mut self, entity: Self::Entity);

    /// Remove target and observer reverse-index entries after every Despawn
    /// callback and the structural despawn have completed.
    fn cleanup_despawned_entity(&mut self, entity: Self::Entity);
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
            adapter.dispatch(LifecycleEvent::Discard, entity, component);
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
            adapter.dispatch(LifecycleEvent::Add, entity, component);
            if !adapter.entity_exists(entity) {
                return LifecycleMutationOutcome::EntityGone;
            }
            if !adapter.component_exists(entity, component) {
                return LifecycleMutationOutcome::ComponentGone;
            }
        }

        adapter.dispatch(LifecycleEvent::Insert, entity, component);
        LifecycleMutationOutcome::Complete
    }

    /// Remove one component, dispatching `Remove` while its old value remains
    /// readable. A callback may satisfy the removal itself; the outer removal
    /// then becomes a no-op.
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

        adapter.dispatch(LifecycleEvent::Remove, entity, component);
        if !adapter.entity_exists(entity) {
            return LifecycleMutationOutcome::EntityGone;
        }
        if adapter.component_exists(entity, component) {
            adapter.remove_component(entity, component);
        }

        LifecycleMutationOutcome::Complete
    }

    /// Despawn an entity after one callback per still-present component.
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
        if !adapter.entity_exists(entity) {
            return LifecycleMutationOutcome::EntityGone;
        }

        for component in components {
            if !adapter.entity_exists(entity) {
                return LifecycleMutationOutcome::EntityGone;
            }
            if !adapter.component_exists(entity, *component) {
                continue;
            }

            adapter.dispatch(LifecycleEvent::Despawn, entity, *component);
        }

        if !adapter.entity_exists(entity) {
            return LifecycleMutationOutcome::EntityGone;
        }

        adapter.despawn_entity(entity);
        adapter.cleanup_despawned_entity(entity);
        LifecycleMutationOutcome::Complete
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::{
        LifecycleEvent, LifecycleMutationAdapter, LifecycleMutationCore, LifecycleMutationOutcome,
    };

    type Entity = u32;
    type Component = u32;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum CallbackAction {
        RemoveComponent,
        DespawnEntity,
        ReinsertComponent,
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
        actions: HashMap<(LifecycleEvent, Component), CallbackAction>,
        steps: Vec<Step>,
    }

    impl FakeAdapter {
        fn new(entity: Entity, components: impl IntoIterator<Item = Component>) -> Self {
            Self {
                entity,
                alive: true,
                components: components.into_iter().collect(),
                actions: HashMap::new(),
                steps: Vec::new(),
            }
        }

        fn on(
            mut self,
            event: LifecycleEvent,
            component: Component,
            action: CallbackAction,
        ) -> Self {
            self.actions.insert((event, component), action);
            self
        }
    }

    impl LifecycleMutationAdapter for FakeAdapter {
        type Entity = Entity;
        type Component = Component;

        fn entity_exists(&self, entity: Entity) -> bool {
            entity == self.entity && self.alive
        }

        fn component_exists(&self, entity: Entity, component: Component) -> bool {
            self.entity_exists(entity) && self.components.contains(&component)
        }

        fn dispatch(&mut self, event: LifecycleEvent, entity: Entity, component: Component) {
            self.steps.push(Step::Dispatch {
                event,
                component,
                readable: self.component_exists(entity, component),
            });
            match self.actions.get(&(event, component)).copied() {
                Some(CallbackAction::RemoveComponent) => {
                    self.components.remove(&component);
                }
                Some(CallbackAction::DespawnEntity) => {
                    self.alive = false;
                    self.components.clear();
                }
                Some(CallbackAction::ReinsertComponent) => {
                    self.components.insert(component);
                }
                None => {}
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
    fn remove_dispatches_while_value_is_readable() {
        let mut adapter = FakeAdapter::new(7, [10]);

        let outcome = LifecycleMutationCore.remove(&mut adapter, 7, 10);

        assert_eq!(outcome, LifecycleMutationOutcome::Complete);
        assert_eq!(
            adapter.steps,
            [
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
    fn despawn_dispatches_once_per_component_before_mutation_and_cleanup() {
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
                Step::Despawn,
                Step::Cleanup,
            ]
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
}
