use std::{
    fmt,
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{component::ComponentId, entity::Entity, world::World},
    prelude::Resource,
};
use pybevy_core::{
    public_error::{NOT_AN_OBSERVER_ENTITY, RESOURCE_ENTITY_DESPAWN},
    registry::global_registry,
};
use pybevy_ecs::shared::{
    observer_registry::{
        LifecycleKind, ObserverEntry as CoreObserverEntry, ObserverEventKey, ObserverFilter,
        ObserverOrigin, ObserverRegistryCore, ObserverTypeKey, ResolvedObserverComponent,
    },
    system_runtime::{ErrorPolicy, execute_observer},
};
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    ffi,
    prelude::*,
    types::PyType,
};

use super::{
    component_type::{ComponentRegistry, PyComponentType},
    observer::EventType,
    resource::hierarchy_contains_resource_entity,
    resource_type::ResourceRegistry,
    system::{SystemFunction, SystemParamType},
    system_interpreter::{MainPreparedObserver, ObserverRuntimeSinks, new_main_observer},
};

/// Main-interpreter state retained by one observer registration.
pub struct ObserverPayload {
    pub(crate) prepared: MainPreparedObserver,
    /// Keep every type object used as a registry key alive until removal.
    /// This prevents CPython from recycling an address still present in the
    /// interpreter-neutral registry.
    _retained_types: Vec<Py<PyType>>,
}

impl fmt::Debug for ObserverPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObserverPayload")
            .field("metadata", &self.prepared.metadata)
            .finish_non_exhaustive()
    }
}

pub type ObserverEntry = CoreObserverEntry<ObserverPayload>;

/// A fully validated observer without a registry entity.
///
/// Reload prepares the whole definition batch before selectively retiring the
/// previous batch, so a bad candidate cannot leave the live scene unobserved.
pub(crate) struct PreparedObserverRegistration {
    prepared: Arc<ObserverPayload>,
    event: ObserverEventKey,
    filter: ObserverFilter,
    target: Option<Entity>,
    origin: ObserverOrigin,
}

/// PyO3 adapter around the interpreter-neutral observer registry.
#[derive(Debug, Default, Resource)]
pub struct ObserverRegistry {
    core: ObserverRegistryCore<ObserverPayload>,
}

impl ObserverRegistry {
    /// Register a global observer and return its observer entity.
    pub fn register_observer(
        py: Python,
        func: &Bound<'_, PyAny>,
        world: &mut World,
    ) -> PyResult<Entity> {
        Self::register(py, func, None, ObserverOrigin::Runtime, world)
    }

    /// Register an observer declared by `App.add_observer`.
    pub fn register_definition_observer(
        py: Python,
        func: &Bound<'_, PyAny>,
        world: &mut World,
    ) -> PyResult<Entity> {
        Self::register(py, func, None, ObserverOrigin::AppDefinition, world)
    }

    /// Register an observer scoped to one target entity.
    pub fn register_observer_for_entity(
        py: Python,
        func: &Bound<'_, PyAny>,
        entity: Entity,
        world: &mut World,
    ) -> PyResult<Entity> {
        Self::register(py, func, Some(entity), ObserverOrigin::Runtime, world)
    }

    fn register(
        py: Python,
        func: &Bound<'_, PyAny>,
        target: Option<Entity>,
        origin: ObserverOrigin,
        world: &mut World,
    ) -> PyResult<Entity> {
        let prepared = Self::prepare(py, func, target, origin, world)?;
        Ok(Self::commit_prepared(prepared, world))
    }

    pub(crate) fn prepare_definition_observer(
        py: Python,
        func: &Bound<'_, PyAny>,
        world: &mut World,
    ) -> PyResult<PreparedObserverRegistration> {
        Self::prepare(py, func, None, ObserverOrigin::AppDefinition, world)
    }

    fn prepare(
        py: Python,
        func: &Bound<'_, PyAny>,
        target: Option<Entity>,
        origin: ObserverOrigin,
        world: &mut World,
    ) -> PyResult<PreparedObserverRegistration> {
        let system_func = SystemFunction::new(py, func.clone())?;
        let (event_type, bundle_filter) = Self::validate_system_function(py, &system_func)?;

        // Resolve every filter before spawning the observer entity. This keeps
        // registration fallibility ahead of the registry's infallible commit
        // and prevents an unresolved filter from becoming less restrictive.
        let (event, filter, retained_types) =
            lower_registration(py, world, &event_type, bundle_filter.as_deref())?;

        let generation = world
            .get_resource::<pybevy_reload::HotReloadGeneration>()
            .map(|generation| generation.current)
            .unwrap_or(0);
        let sinks = world
            .get_resource::<ObserverRuntimeSinks>()
            .cloned()
            .unwrap_or_else(|| ObserverRuntimeSinks {
                error_state: Arc::new(Mutex::new(Vec::new())),
                error_buffer: Arc::new(Mutex::new(None)),
            });
        let prepared = new_main_observer(
            system_func,
            generation,
            sinks.error_state,
            sinks.error_buffer,
        );
        Ok(PreparedObserverRegistration {
            prepared: Arc::new(ObserverPayload {
                prepared,
                _retained_types: retained_types,
            }),
            event,
            filter,
            target,
            origin,
        })
    }

    fn commit_prepared(prepared: PreparedObserverRegistration, world: &mut World) -> Entity {
        if !world.contains_resource::<ObserverRegistry>() {
            world.insert_resource(ObserverRegistry::default());
        }

        let observer_entity = world.spawn_empty().id();
        let entry = ObserverEntry {
            observer_entity,
            prepared: prepared.prepared,
            event: prepared.event,
            filter: prepared.filter,
            target: prepared.target,
            origin: prepared.origin,
        };

        let insert_result = world.resource_mut::<ObserverRegistry>().core.insert(entry);
        insert_result.expect("a freshly spawned observer entity is unique");

        observer_entity
    }

    /// Commit a prevalidated entrypoint observer batch.
    ///
    /// Full reload preserves its existing all-observer reset. Partial reload
    /// replaces only App definitions and leaves World/entity observers intact.
    pub(crate) fn replace_definition_observers(
        prepared: Vec<PreparedObserverRegistration>,
        clear_runtime: bool,
        world: &mut World,
    ) {
        let old_entries = world
            .get_resource_mut::<ObserverRegistry>()
            .map(|mut registry| registry.clear_all())
            .unwrap_or_default();
        let (retired_entries, runtime_entries): (Vec<_>, Vec<_>) = old_entries
            .into_iter()
            .partition(|entry| clear_runtime || entry.origin == ObserverOrigin::AppDefinition);

        for entry in &retired_entries {
            if world.get_entity(entry.observer_entity).is_ok() {
                world.despawn(entry.observer_entity);
            }
        }

        for registration in prepared {
            Self::commit_prepared(registration, world);
        }

        if !runtime_entries.is_empty() && !world.contains_resource::<ObserverRegistry>() {
            world.insert_resource(ObserverRegistry::default());
        }
        for entry in runtime_entries {
            world
                .resource_mut::<ObserverRegistry>()
                .core
                .insert(entry)
                .expect("a retained runtime observer entity stays unique");
        }

        // Prepared Python handles drop only after registry borrows and entity
        // mutations have ended, so finalizers may safely re-enter.
        drop(retired_entries);
    }

    /// Validate the parts of an observer that do not require World access.
    pub(crate) fn validate_observer_signature(py: Python, func: &Bound<'_, PyAny>) -> PyResult<()> {
        let system_func = SystemFunction::new_uncached(py, func.clone())?;
        Self::validate_system_function(py, &system_func)?;
        Ok(())
    }

    fn validate_system_function(
        py: Python,
        system_func: &SystemFunction,
    ) -> PyResult<(EventType, Option<Vec<PyComponentType>>)> {
        let event = Self::extract_event_type_from_params(system_func)?;

        // Observers bypass add_systems' validation gate, so reject aliasing
        // parameter combinations before mutating the World or registry.
        crate::ecs::dynamic_system::validate_system_params(&system_func.params, "observer", py)?;

        Ok(event)
    }

    /// Invoke one owned registry snapshot through the neutral observer shell.
    pub(crate) fn invoke(
        entry: &ObserverEntry,
        world: &mut World,
        trigger: &Py<super::observer::PyOn>,
        target: Option<Entity>,
        policy: ErrorPolicy,
    ) -> PyResult<()> {
        let prepared = &entry.prepared.prepared;
        let current_generation = world
            .get_resource::<pybevy_reload::HotReloadGeneration>()
            .map(|generation| generation.current);
        // SAFETY: dispatch owns the exclusive World, the registry entry is an
        // owned snapshot, and the shared shell owns the callback validity and
        // command queue through invalidation and application.
        let result = unsafe {
            execute_observer(
                &prepared.interpreter,
                &prepared.retained,
                &prepared.params,
                &prepared.persistent,
                &prepared.failure_sink,
                &prepared.metadata,
                current_generation,
                trigger,
                target,
                policy,
                world,
            )
        };
        result.map_err(|mut failure| {
            failure
                .exception
                .take()
                .unwrap_or_else(|| PyRuntimeError::new_err(failure.report.message))
        })
    }

    fn extract_event_type_from_params(
        system_func: &SystemFunction,
    ) -> PyResult<(EventType, Option<Vec<PyComponentType>>)> {
        for param in &system_func.params {
            if let SystemParamType::On {
                event_type,
                bundle_filter,
            } = &param.ty
            {
                return Ok((event_type.clone(), bundle_filter.clone()));
            }
        }

        Err(PyTypeError::new_err(
            "Observer function must have an On[EventType] parameter",
        ))
    }

    /// Snapshot matching user-event observers in registration order.
    ///
    /// The returned entries own only cloned Arcs. Callers must perform the
    /// component filter check immediately before each callback so mutations by
    /// an earlier observer remain visible to later observers.
    #[must_use]
    pub fn snapshot_user_event(
        &self,
        event: &Bound<'_, PyAny>,
        target: Option<Entity>,
    ) -> Vec<ObserverEntry> {
        let key =
            ObserverEventKey::User(ObserverTypeKey::new(event.get_type().as_type_ptr() as usize));
        self.core.snapshot(key, target)
    }

    /// Snapshot observers for one exact component lifecycle transition.
    #[must_use]
    pub fn snapshot_lifecycle(
        &self,
        lifecycle: LifecycleKind,
        component_type: &PyComponentType,
        component_id: ComponentId,
        target: Entity,
    ) -> Vec<ObserverEntry> {
        let type_key = component_type_key(component_type);
        self.core
            .snapshot(ObserverEventKey::Lifecycle(lifecycle), Some(target))
            .into_iter()
            .filter(|entry| {
                entry
                    .filter
                    .matches_lifecycle_component(type_key, component_id)
            })
            .collect()
    }

    /// Apply a user-event component filter against the target's current state.
    #[must_use]
    pub fn matches_user_filter(
        entry: &ObserverEntry,
        world: &World,
        target: Option<Entity>,
    ) -> bool {
        entry
            .filter
            .matches_user_target(target, |entity, component_id| {
                world
                    .get_entity(entity)
                    .is_ok_and(|entity_ref| entity_ref.contains_id(component_id))
            })
    }

    /// Resolve an already-registered component without structurally mutating
    /// the World. Lifecycle dispatch uses this to pair the backend type key
    /// with the same ECS id captured during observer registration.
    #[must_use]
    pub fn component_id(world: &World, component_type: &PyComponentType) -> Option<ComponentId> {
        match component_type {
            PyComponentType::Dynamic(type_ptr) => global_registry::get_bridge_by_py_type(*type_ptr)
                .and_then(|bridge| world.components().get_id(bridge.bevy_type_id())),
            PyComponentType::Resource(type_ptr) => {
                if let Some(bridge) = global_registry::get_resource_bridge_by_py_type(*type_ptr) {
                    bridge.resource_id(world)
                } else {
                    world
                        .get_resource::<ResourceRegistry>()
                        .and_then(|registry| registry.get(*type_ptr as usize))
                }
            }
            PyComponentType::Custom(type_ptr) => world
                .get_resource::<ComponentRegistry>()
                .and_then(|registry| registry.get(*type_ptr as usize)),
        }
    }

    /// Remove one observer and return its complete entry for out-of-borrow drop.
    pub fn remove_observer(&mut self, observer_entity: Entity) -> Option<ObserverEntry> {
        self.core.remove(observer_entity)
    }

    /// Drain all observers for hot reload.
    pub(crate) fn clear_all(&mut self) -> Vec<ObserverEntry> {
        self.core.clear()
    }

    /// Remove every observer scoped to a despawning target.
    fn remove_for_entity(&mut self, watched_entity: Entity) -> Vec<ObserverEntry> {
        self.core.remove_for_target(watched_entity)
    }

    /// Despawn an observer entity and remove its prepared registration.
    pub fn despawn_observer(observer_entity: Entity, world: &mut World) -> PyResult<()> {
        if hierarchy_contains_resource_entity(world, observer_entity) {
            return Err(PyTypeError::new_err(RESOURCE_ENTITY_DESPAWN));
        }

        let removed = world
            .get_resource_mut::<ObserverRegistry>()
            .and_then(|mut registry| registry.remove_observer(observer_entity));

        if removed.is_none() {
            // Registry membership identifies Python observer entities.
            if world.entities().contains(observer_entity) {
                return Err(PyValueError::new_err(NOT_AN_OBSERVER_ENTITY));
            }
            return Ok(());
        }

        if let Ok(entity_mut) = world.get_entity_mut(observer_entity) {
            entity_mut.despawn();
        }

        // `removed` drops here, after the registry resource borrow and entity
        // mutation have both ended. A Python finalizer may safely re-enter.
        drop(removed);
        Ok(())
    }

    /// Remove and despawn observers scoped to a despawning target entity.
    pub fn cleanup_on_entity_despawn(watched_entity: Entity, world: &mut World) {
        let removed = world
            .get_resource_mut::<ObserverRegistry>()
            .map(|mut registry| registry.remove_for_entity(watched_entity))
            .unwrap_or_default();

        for entry in &removed {
            if let Ok(entity_mut) = world.get_entity_mut(entry.observer_entity) {
                entity_mut.despawn();
            }
        }

        // Drop prepared Python handles only after releasing the resource borrow.
        drop(removed);
    }
}

fn lower_registration(
    py: Python,
    world: &mut World,
    event_type: &EventType,
    bundle_filter: Option<&[PyComponentType]>,
) -> PyResult<(ObserverEventKey, ObserverFilter, Vec<Py<PyType>>)> {
    let (event, components, mut retained_types) = match event_type {
        EventType::Custom(event_type) => (
            ObserverEventKey::User(ObserverTypeKey::new(
                event_type.bind(py).as_type_ptr() as usize
            )),
            bundle_filter.unwrap_or_default(),
            vec![event_type.clone_ref(py)],
        ),
        EventType::Add(component) => (
            ObserverEventKey::Lifecycle(LifecycleKind::Add),
            std::slice::from_ref(component),
            Vec::new(),
        ),
        EventType::Insert(component) => (
            ObserverEventKey::Lifecycle(LifecycleKind::Insert),
            std::slice::from_ref(component),
            Vec::new(),
        ),
        EventType::Remove(component) => (
            ObserverEventKey::Lifecycle(LifecycleKind::Remove),
            std::slice::from_ref(component),
            Vec::new(),
        ),
        EventType::Discard(component) => (
            ObserverEventKey::Lifecycle(LifecycleKind::Discard),
            std::slice::from_ref(component),
            Vec::new(),
        ),
        EventType::Despawn(component) => (
            ObserverEventKey::Lifecycle(LifecycleKind::Despawn),
            std::slice::from_ref(component),
            Vec::new(),
        ),
    };

    let mut resolved = Vec::with_capacity(components.len());
    for component in components {
        let component_id = component.register_simple(world, py);
        retained_types.push(retain_component_type(py, component)?);
        resolved.push(ResolvedObserverComponent {
            type_key: component_type_key(component),
            component_id,
        });
    }

    Ok((event, ObserverFilter::new(resolved), retained_types))
}

fn component_type_key(component: &PyComponentType) -> ObserverTypeKey {
    let type_ptr = match component {
        PyComponentType::Dynamic(type_ptr)
        | PyComponentType::Resource(type_ptr)
        | PyComponentType::Custom(type_ptr) => *type_ptr,
    };
    ObserverTypeKey::new(type_ptr as usize)
}

fn retain_component_type(py: Python, component: &PyComponentType) -> PyResult<Py<PyType>> {
    let type_ptr = match component {
        PyComponentType::Dynamic(type_ptr)
        | PyComponentType::Resource(type_ptr)
        | PyComponentType::Custom(type_ptr) => *type_ptr,
    };
    // SAFETY: `PyComponentType` is created only from a live Python type object.
    // We immediately create a new strong reference and retain it in the
    // observer payload until the registry entry is removed.
    let type_object = unsafe { Bound::from_borrowed_ptr(py, type_ptr as *mut ffi::PyObject) };
    Ok(type_object.cast::<PyType>()?.clone().unbind())
}
