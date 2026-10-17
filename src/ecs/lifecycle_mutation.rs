//! Main/PyO3 adapter for the interpreter-neutral lifecycle planner.

use std::{cell::RefCell, collections::HashSet};

use bevy::ecs::{
    component::ComponentId,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    world::{World, WorldId},
};
use pybevy_core::registry::global_registry;
use pybevy_ecs::shared::{
    lifecycle_mutation::{
        LifecycleEvent, LifecycleMutationAdapter, LifecycleMutationCore, LifecycleMutationOutcome,
        RecursiveDespawnSnapshot, required_component_ids,
    },
    observer_registry::LifecycleKind,
    system_runtime::ErrorPolicy,
};
use pyo3::prelude::*;
use smallvec::SmallVec;

use super::{
    component_type::PyComponentType,
    observer::{PyAdd, PyDespawn, PyDiscard, PyInsert, PyOn, PyRemove},
    observer_registry::{ObserverEntry, ObserverRegistry},
    resource::is_resource_entity,
    world::PyWorld,
};

type NoStructuralInsert = fn(&mut World) -> bool;
type ActiveLifecycleKey = (WorldId, LifecycleEvent, Entity, usize, Entity);
type ResolvedComponentIds = SmallVec<[(PyComponentType, ComponentId); 8]>;

thread_local! {
    // Bound same-key recursion per observer without dropping independent callbacks.
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
        PyComponentType::Dynamic(type_ptr)
        | PyComponentType::Resource(type_ptr)
        | PyComponentType::Custom(type_ptr) => type_ptr as usize,
    }
}

struct MainLifecycleAdapter<'a, F = NoStructuralInsert> {
    world: &'a mut World,
    insert: Option<F>,
    despawn_result: Option<(Entity, bool)>,
    component_ids: RefCell<ResolvedComponentIds>,
}

impl<'a> MainLifecycleAdapter<'a, NoStructuralInsert> {
    fn without_insert(world: &'a mut World, despawn_result: Option<(Entity, bool)>) -> Self {
        Self {
            world,
            insert: None,
            despawn_result,
            component_ids: RefCell::new(ResolvedComponentIds::new()),
        }
    }
}

impl<'a, F: FnOnce(&mut World) -> bool> MainLifecycleAdapter<'a, F> {
    fn with_insert(world: &'a mut World, insert: F) -> Self {
        Self {
            world,
            insert: Some(insert),
            despawn_result: None,
            component_ids: RefCell::new(ResolvedComponentIds::new()),
        }
    }

    fn component_id(&self, component: PyComponentType) -> Option<ComponentId> {
        let known = self
            .component_ids
            .borrow()
            .iter()
            .find(|(resolved, _)| *resolved == component)
            .map(|(_, id)| *id);
        if known.is_some() {
            return known;
        }
        let id = ObserverRegistry::component_id(self.world, &component)?;
        self.component_ids.borrow_mut().push((component, id));
        Some(id)
    }

    fn active_key(
        &self,
        event: LifecycleEvent,
        entity: Entity,
        component: PyComponentType,
        observer_entity: Entity,
    ) -> ActiveLifecycleKey {
        (
            self.world.id(),
            event,
            entity,
            component_key(component),
            observer_entity,
        )
    }
}

impl<F: FnOnce(&mut World) -> bool> LifecycleMutationAdapter for MainLifecycleAdapter<'_, F> {
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

    fn required_components(&mut self, components: &[PyComponentType]) -> Vec<PyComponentType> {
        let explicit = components
            .iter()
            .filter_map(|component| {
                if let PyComponentType::Dynamic(ptr) = component {
                    global_registry::get_bridge_by_py_type(*ptr)
                        .map(|bridge| bridge.register(self.world))
                } else {
                    self.component_id(*component)
                }
            })
            .collect::<Vec<_>>();
        required_component_ids(self.world.components(), &explicit)
            .into_iter()
            .filter_map(|id| native_component_type(self.world, id))
            .collect()
    }

    fn snapshot_observers(
        &mut self,
        event: LifecycleEvent,
        entity: Entity,
        component: PyComponentType,
    ) -> Vec<ObserverEntry> {
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
        let key = self.active_key(event, entity, component, observer.observer_entity);
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
                    propagation: None,
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
        // A lifecycle callback can parent a resource entity under this root
        // after the boundary check, so detach any before Bevy's cascade runs.
        detach_resource_descendants(self.world, entity);
        let result = self.world.despawn(entity);
        if let Some((root, despawned)) = &mut self.despawn_result
            && *root == entity
        {
            *despawned |= result;
        }
    }

    fn cleanup_despawned_entity(&mut self, entity: Entity) {
        ObserverRegistry::cleanup_on_entity_despawn(entity, self.world);
    }
}

/// Break the parent link of every resource entity under `root` so Bevy's
/// relationship cascade cannot despawn resource storage.
fn detach_resource_descendants(world: &mut World, root: Entity) {
    let mut pending = vec![root];
    let mut seen = HashSet::new();
    let mut detach = Vec::new();
    while let Some(entity) = pending.pop() {
        if !seen.insert(entity) || !world.entities().contains(entity) {
            continue;
        }
        if entity != root && is_resource_entity(world, entity) {
            detach.push(entity);
            continue;
        }
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
    }
    for entity in detach {
        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
            entity_mut.remove::<ChildOf>();
        }
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

pub(crate) fn native_component_type(world: &World, id: ComponentId) -> Option<PyComponentType> {
    let native_type = world.components().get_info(id)?.type_id()?;
    let bridge = global_registry::get_component_bridge_by_type_id(native_type)?;
    Some(PyComponentType::Dynamic(bridge.py_type_ptr()))
}

pub(crate) fn insert_many_with(
    world: &mut World,
    entity: Entity,
    components: &[PyComponentType],
    insert: impl FnOnce(&mut World) -> bool,
) -> LifecycleMutationOutcome {
    LifecycleMutationCore.insert_many(
        &mut MainLifecycleAdapter::with_insert(world, insert),
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
        &mut MainLifecycleAdapter::without_insert(world, None),
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
        &mut MainLifecycleAdapter::without_insert(world, None),
        entity,
        components,
    )
}

/// Snapshot the complete relationship cascade before any Python callback, then
/// dispatch/clean every member before Bevy may structurally cascade the root.
pub(crate) fn despawn_recursive(world: &mut World, root: Entity) -> bool {
    let mut pending = vec![root];
    let mut seen = HashSet::new();
    let mut snapshots = Vec::new();
    while let Some(entity) = pending.pop() {
        if !seen.insert(entity) || !world.entities().contains(entity) {
            continue;
        }
        // Never cascade into resource storage, even if a resource entity was
        // parented under the despawn root.
        if is_resource_entity(world, entity) {
            continue;
        }
        let children = world
            .get::<Children>(entity)
            .map(|children| children.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        // Reverse the stack push so traversal follows Bevy's stored child order.
        pending.extend(children.into_iter().rev());
        snapshots.push(RecursiveDespawnSnapshot::new(
            entity,
            PyWorld::get_entity_data_names(world, entity),
        ));
    }
    if snapshots.is_empty() && !is_resource_entity(world, root) {
        snapshots.push(RecursiveDespawnSnapshot::new(root, Vec::new()));
    }
    let mut adapter = MainLifecycleAdapter::without_insert(world, Some((root, false)));
    LifecycleMutationCore.despawn_recursive(&mut adapter, &snapshots);
    adapter
        .despawn_result
        .is_some_and(|(_, despawned)| despawned)
}
