//! Main/PyO3 adapter for the interpreter-neutral lifecycle planner.

use std::{cell::RefCell, collections::HashSet};

use bevy::ecs::{
    component::ComponentId,
    entity::Entity,
    world::{World, WorldId},
};
use pybevy_ecs::shared::{
    lifecycle_mutation::{
        LifecycleEvent, LifecycleMutationAdapter, LifecycleMutationCore, LifecycleMutationOutcome,
        RecursiveDespawnSnapshot,
    },
    observer_registry::LifecycleKind,
    system_runtime::ErrorPolicy,
};
use pyo3::prelude::*;

use super::{
    component_type::PyComponentType,
    observer::{PyAdd, PyDespawn, PyDiscard, PyInsert, PyOn, PyRemove},
    observer_registry::{ObserverEntry, ObserverRegistry},
};

type StructuralInsert<'a> = Box<dyn FnOnce(&mut World) -> bool + 'a>;
type ActiveLifecycleKey = (WorldId, LifecycleEvent, Entity, usize);

thread_local! {
    /// Prevent one callback from recursively re-emitting the exact lifecycle
    /// event that is already delivering it in the same World. Other Worlds
    /// and event kinds remain fully reentrant (for example Despawn -> explicit
    /// remove -> Discard/Remove).
    static ACTIVE_LIFECYCLE_CALLBACKS: RefCell<HashSet<ActiveLifecycleKey>> =
        RefCell::new(HashSet::new());
}

struct ActiveLifecycleGuard(ActiveLifecycleKey);

impl Drop for ActiveLifecycleGuard {
    fn drop(&mut self) {
        ACTIVE_LIFECYCLE_CALLBACKS.with(|active| {
            active.borrow_mut().remove(&self.0);
        });
    }
}

fn component_key(component: PyComponentType) -> usize {
    match component {
        PyComponentType::Dynamic(type_ptr) | PyComponentType::Custom(type_ptr) => type_ptr as usize,
    }
}

struct MainLifecycleAdapter<'a> {
    world: &'a mut World,
    insert: Option<StructuralInsert<'a>>,
}

impl MainLifecycleAdapter<'_> {
    fn component_id(&self, component: PyComponentType) -> Option<ComponentId> {
        ObserverRegistry::component_id(self.world, &component)
    }

    fn active_key(
        &self,
        event: LifecycleEvent,
        entity: Entity,
        component: PyComponentType,
    ) -> ActiveLifecycleKey {
        (self.world.id(), event, entity, component_key(component))
    }
}

impl LifecycleMutationAdapter for MainLifecycleAdapter<'_> {
    type Entity = Entity;
    type Component = PyComponentType;
    type Observer = ObserverEntry;

    fn entity_exists(&self, entity: Entity) -> bool {
        self.world.entities().contains(entity)
    }

    fn component_exists(&self, entity: Entity, component: PyComponentType) -> bool {
        let Some(component_id) = self.component_id(component) else {
            return false;
        };
        self.world
            .get_entity(entity)
            .is_ok_and(|entity_ref| entity_ref.contains_id(component_id))
    }

    fn snapshot_observers(
        &mut self,
        event: LifecycleEvent,
        entity: Entity,
        component: PyComponentType,
    ) -> Vec<ObserverEntry> {
        let key = self.active_key(event, entity, component);
        if ACTIVE_LIFECYCLE_CALLBACKS.with(|active| active.borrow().contains(&key)) {
            return Vec::new();
        }
        let Some(component_id) = self.component_id(component) else {
            return Vec::new();
        };
        self.world
            .get_resource::<ObserverRegistry>()
            .map(|registry| {
                registry.snapshot_lifecycle(lifecycle_kind(event), &component, component_id, entity)
            })
            .unwrap_or_default()
    }

    fn invoke_observer(
        &mut self,
        observer: &ObserverEntry,
        event: LifecycleEvent,
        entity: Entity,
        component: PyComponentType,
    ) {
        let key = self.active_key(event, entity, component);
        let inserted = ACTIVE_LIFECYCLE_CALLBACKS.with(|active| active.borrow_mut().insert(key));
        if !inserted {
            return;
        }
        let _active_guard = ActiveLifecycleGuard(key);
        Python::attach(|py| {
            let event_data: Py<PyAny> = match event {
                LifecycleEvent::Add => Py::new(py, PyAdd).map(Py::into_any),
                LifecycleEvent::Insert => Py::new(py, PyInsert).map(Py::into_any),
                LifecycleEvent::Remove => Py::new(py, PyRemove).map(Py::into_any),
                LifecycleEvent::Discard => Py::new(py, PyDiscard).map(Py::into_any),
                LifecycleEvent::Despawn => Py::new(py, PyDespawn).map(Py::into_any),
            }
            .expect("lifecycle marker allocation failed");
            let trigger = Py::new(
                py,
                PyOn {
                    event_data,
                    entity: Some(entity),
                },
            )
            .expect("lifecycle On allocation failed");
            let _ = ObserverRegistry::invoke(
                observer,
                self.world,
                &trigger,
                Some(entity),
                ErrorPolicy::ReportAndContinue,
            );
        });
    }

    fn insert_component(&mut self, _entity: Entity, _component: PyComponentType) -> bool {
        if let Some(insert) = self.insert.take() {
            insert(self.world)
        } else {
            false
        }
    }

    fn insert_components(&mut self, _entity: Entity, _components: &[PyComponentType]) -> bool {
        if let Some(insert) = self.insert.take() {
            insert(self.world)
        } else {
            false
        }
    }

    fn remove_component(&mut self, entity: Entity, component: PyComponentType) {
        if let Some(component_id) = self.component_id(component)
            && let Ok(mut entity_mut) = self.world.get_entity_mut(entity)
        {
            entity_mut.remove_by_id(component_id);
        }
    }

    fn despawn_entity(&mut self, entity: Entity) {
        self.world.despawn(entity);
    }

    fn cleanup_despawned_entity(&mut self, entity: Entity) {
        ObserverRegistry::cleanup_on_entity_despawn(entity, self.world);
    }
}

fn lifecycle_kind(event: LifecycleEvent) -> LifecycleKind {
    match event {
        LifecycleEvent::Add => LifecycleKind::Add,
        LifecycleEvent::Insert => LifecycleKind::Insert,
        LifecycleEvent::Remove => LifecycleKind::Remove,
        LifecycleEvent::Discard => LifecycleKind::Discard,
        LifecycleEvent::Despawn => LifecycleKind::Despawn,
    }
}

pub(crate) fn insert_many_with(
    world: &mut World,
    entity: Entity,
    components: &[PyComponentType],
    insert: impl FnOnce(&mut World) -> bool,
) -> LifecycleMutationOutcome {
    LifecycleMutationCore.insert_many(
        &mut MainLifecycleAdapter {
            world,
            insert: Some(Box::new(insert)),
        },
        entity,
        components,
    )
}

pub(crate) fn remove(
    world: &mut World,
    entity: Entity,
    component: PyComponentType,
) -> LifecycleMutationOutcome {
    LifecycleMutationCore.remove(
        &mut MainLifecycleAdapter {
            world,
            insert: None,
        },
        entity,
        component,
    )
}

pub(crate) fn finish_new_bundle(
    world: &mut World,
    entity: Entity,
    components: &[PyComponentType],
) -> LifecycleMutationOutcome {
    LifecycleMutationCore.finish_new_bundle(
        &mut MainLifecycleAdapter {
            world,
            insert: None,
        },
        entity,
        components,
    )
}

/// Snapshot the complete relationship cascade before any Python callback, then
/// dispatch/clean every member before Bevy may structurally cascade the root.
pub(crate) fn despawn_recursive(world: &mut World, root: Entity) -> LifecycleMutationOutcome {
    let mut pending = vec![root];
    let mut seen = HashSet::new();
    let mut snapshots = Vec::new();
    while let Some(entity) = pending.pop() {
        if !seen.insert(entity) || !world.entities().contains(entity) {
            continue;
        }
        let children = world
            .get::<bevy::ecs::hierarchy::Children>(entity)
            .map(|children| children.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        // Reverse the stack push so traversal follows Bevy's stored child order.
        pending.extend(children.into_iter().rev());
        snapshots.push(RecursiveDespawnSnapshot::new(
            entity,
            super::world::PyWorld::get_entity_data_names(world, entity),
        ));
    }
    if snapshots.is_empty() {
        snapshots.push(RecursiveDespawnSnapshot::new(root, Vec::new()));
    }
    LifecycleMutationCore.despawn_recursive(
        &mut MainLifecycleAdapter {
            world,
            insert: None,
        },
        &snapshots,
    )
}
