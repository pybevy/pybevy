use std::{
    fmt,
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{component::ComponentId, entity::Entity, world::World},
    prelude::Resource,
};
use pybevy_core::registry::global_registry;
use pybevy_ecs::shared::{
    observer_registry::{
        LifecycleKind, ObserverEntry as CoreObserverEntry, ObserverEventKey, ObserverFilter,
        ObserverRegistryCore, ObserverTypeKey, ResolvedObserverComponent,
    },
    system_runtime::{ErrorPolicy, execute_observer},
};
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError},
    ffi,
    prelude::*,
    types::PyType,
};

use super::{
    component_type::{ComponentRegistry, PyComponentType},
    observer::EventType,
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
        Self::register(py, func, None, world)
    }

    /// Register an observer scoped to one target entity.
    pub fn register_observer_for_entity(
        py: Python,
        func: &Bound<'_, PyAny>,
        entity: Entity,
        world: &mut World,
    ) -> PyResult<Entity> {
        Self::register(py, func, Some(entity), world)
    }

    fn register(
        py: Python,
        func: &Bound<'_, PyAny>,
        target: Option<Entity>,
        world: &mut World,
    ) -> PyResult<Entity> {
        let system_func = SystemFunction::new(py, func.clone())?;
        let (event_type, bundle_filter) = Self::validate_system_function(py, &system_func)?;

        // Resolve every filter before spawning the observer entity. This keeps
        // registration fallibility ahead of the registry's infallible commit
        // and prevents an unresolved filter from becoming less restrictive.
        let (event, filter, retained_types) =
            lower_registration(py, world, &event_type, bundle_filter.as_deref())?;

        if !world.contains_resource::<ObserverRegistry>() {
            world.insert_resource(ObserverRegistry::default());
        }

        let observer_entity = world.spawn_empty().id();
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
        let entry = ObserverEntry {
            observer_entity,
            prepared: Arc::new(ObserverPayload {
                prepared,
                _retained_types: retained_types,
            }),
            event,
            filter,
            target,
        };

        let insert_result = world.resource_mut::<ObserverRegistry>().core.insert(entry);
        if let Err(error) = insert_result {
            // A freshly spawned entity cannot already be registered. Fail
            // closed if registry invariants are ever violated.
            world.despawn(observer_entity);
            return Err(PyRuntimeError::new_err(error.to_string()));
        }

        Ok(observer_entity)
    }

    /// Validate the parts of an observer that do not require World access.
    pub(crate) fn validate_observer_signature(py: Python, func: &Bound<'_, PyAny>) -> PyResult<()> {
        let system_func = SystemFunction::new(py, func.clone())?;
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
        let removed = world
            .get_resource_mut::<ObserverRegistry>()
            .and_then(|mut registry| registry.remove_observer(observer_entity));

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
