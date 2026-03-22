use pyo3::{PyTypeInfo, exceptions::PyTypeError, prelude::*, types::PyType};

use super::{PyEntity, component_type::PyComponentType};

/// Base class for all events in PyBevy.
///
/// Events can be triggered immediately via World.trigger() or Commands.trigger().
/// Observers watching for these events will run when they are triggered.
///
/// # Example
/// ```python
/// from dataclasses import dataclass
/// from pybevy.ecs import Event, Entity
///
/// @dataclass
/// class PlayerDied(Event):
///     player_id: int
///     cause: str
///
/// # Entity-targeted event (has 'entity' field)
/// @dataclass
/// class Explode(Event):
///     entity: Entity
///     radius: float
/// ```
#[pyclass(name = "Event", subclass)]
#[derive(Debug, Clone)]
pub struct PyEvent;

#[pymethods]
impl PyEvent {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    pub fn new(
        _args: &Bound<'_, pyo3::types::PyTuple>,
        _kwargs: Option<&Bound<'_, pyo3::types::PyDict>>,
    ) -> Self {
        PyEvent
    }
}

/// Marker class for component addition lifecycle events.
/// Use with On[OnAdd, ComponentType] to observe when components are added.
#[pyclass(name = "OnAdd")]
#[derive(Debug, Clone)]
pub struct PyOnAdd;

/// Marker class for component insertion lifecycle events.
/// Use with On[OnInsert, ComponentType] to observe when components are inserted.
#[pyclass(name = "OnInsert")]
#[derive(Debug, Clone)]
pub struct PyOnInsert;

/// Marker class for component removal lifecycle events.
/// Use with On[OnRemove, ComponentType] to observe when components are removed.
#[pyclass(name = "OnRemove")]
#[derive(Debug, Clone)]
pub struct PyOnRemove;

/// Marker class for component replacement lifecycle events.
/// Use with On[OnReplace, ComponentType] to observe when components are replaced.
#[pyclass(name = "OnReplace")]
#[derive(Debug, Clone)]
pub struct PyOnReplace;

/// Marker class for entity despawn lifecycle events.
/// Use with On[OnDespawn, ComponentType] to observe when entities with the component are despawned.
#[pyclass(name = "OnDespawn")]
#[derive(Debug, Clone)]
pub struct PyOnDespawn;

/// System parameter for observers that provides access to the triggered event.
///
/// # Type Parameters
/// - `E`: The event type (required)
/// - `B`: Optional bundle filter - observer only triggers if entity has these components
///
/// # Example
/// ```python
/// from pybevy.ecs import On, Event
///
/// # Simple observer (no bundle filter)
/// def on_player_died(trigger: On[PlayerDied]) -> None:
///     event = trigger.event()
///     print(f"Player died: {event.player_id}")
///
/// # With bundle filter - only triggers for entities with Mine component
/// def on_explode(trigger: On[Explode, Mine]) -> None:
///     entity = trigger.entity()
///     event = trigger.event()
///     print(f"Mine {entity} exploded")
/// ```
#[pyclass(name = "On", frozen)]
pub struct PyOn {
    pub(crate) event_data: Py<PyAny>,
    pub(crate) entity: Option<bevy::ecs::entity::Entity>,
}

#[pymethods]
impl PyOn {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        _cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = key.py();

        // Check if key is a tuple (On[E, B]) or single type (On[E])
        if let Ok(tuple) = key.cast_exact::<pyo3::types::PyTuple>() {
            // On[E, B] - could be event type with bundle filter OR lifecycle event with component
            if tuple.len() != 2 {
                return Err(PyTypeError::new_err(
                    "On expects 1 or 2 type parameters: On[EventType] or On[EventType, BundleType] or On[LifecycleEvent, ComponentType]",
                ));
            }

            let first_key = tuple.get_item(0)?;
            let second_key = tuple.get_item(1)?;

            // Check if first parameter is a lifecycle event marker
            let first_type_obj = first_key.cast_exact::<PyType>()?;

            // Check for lifecycle event markers
            if first_type_obj.is(&PyOnAdd::type_object(py)) {
                // On[OnAdd, Component] - component addition lifecycle event
                let comp_type = if let Ok(comp_type_obj) = second_key.cast_exact::<PyType>() {
                    PyComponentType::try_from((comp_type_obj, py))?
                } else {
                    return Err(PyTypeError::new_err(
                        "Second parameter to On[OnAdd, ...] must be a Component type",
                    ));
                };
                return Py::new(
                    py,
                    PyOnTypeParam {
                        event_type: EventType::Add(comp_type),
                        bundle_filter: None,
                    },
                )
                .map(|obj| obj.into_any());
            } else if first_type_obj.is(&PyOnInsert::type_object(py)) {
                let comp_type = if let Ok(comp_type_obj) = second_key.cast_exact::<PyType>() {
                    PyComponentType::try_from((comp_type_obj, py))?
                } else {
                    return Err(PyTypeError::new_err(
                        "Second parameter to On[OnInsert, ...] must be a Component type",
                    ));
                };
                return Py::new(
                    py,
                    PyOnTypeParam {
                        event_type: EventType::Insert(comp_type),
                        bundle_filter: None,
                    },
                )
                .map(|obj| obj.into_any());
            } else if first_type_obj.is(&PyOnRemove::type_object(py)) {
                let comp_type = if let Ok(comp_type_obj) = second_key.cast_exact::<PyType>() {
                    PyComponentType::try_from((comp_type_obj, py))?
                } else {
                    return Err(PyTypeError::new_err(
                        "Second parameter to On[OnRemove, ...] must be a Component type",
                    ));
                };
                return Py::new(
                    py,
                    PyOnTypeParam {
                        event_type: EventType::Remove(comp_type),
                        bundle_filter: None,
                    },
                )
                .map(|obj| obj.into_any());
            } else if first_type_obj.is(&PyOnReplace::type_object(py)) {
                let comp_type = if let Ok(comp_type_obj) = second_key.cast_exact::<PyType>() {
                    PyComponentType::try_from((comp_type_obj, py))?
                } else {
                    return Err(PyTypeError::new_err(
                        "Second parameter to On[OnReplace, ...] must be a Component type",
                    ));
                };
                return Py::new(
                    py,
                    PyOnTypeParam {
                        event_type: EventType::Replace(comp_type),
                        bundle_filter: None,
                    },
                )
                .map(|obj| obj.into_any());
            } else if first_type_obj.is(&PyOnDespawn::type_object(py)) {
                let comp_type = if let Ok(comp_type_obj) = second_key.cast_exact::<PyType>() {
                    PyComponentType::try_from((comp_type_obj, py))?
                } else {
                    return Err(PyTypeError::new_err(
                        "Second parameter to On[OnDespawn, ...] must be a Component type",
                    ));
                };
                return Py::new(
                    py,
                    PyOnTypeParam {
                        event_type: EventType::Despawn(comp_type),
                        bundle_filter: None,
                    },
                )
                .map(|obj| obj.into_any());
            }

            // Not a lifecycle event, try regular event with bundle filter
            // Extract event type
            let event_type = EventType::from_py_type(py, first_type_obj)?;

            // Extract bundle filter (component type or tuple of component types)
            let bundle_filter = if let Ok(bundle_type_obj) = second_key.cast_exact::<PyType>() {
                // Single component: On[Event, Component]
                let comp_type = PyComponentType::try_from((bundle_type_obj, py))?;
                Some(vec![comp_type])
            } else if let Ok(origin) = second_key.getattr("__origin__") {
                // Multiple components: On[Event, tuple[A, B, C]] (GenericAlias)
                // Check if origin is tuple type
                if origin.is_instance_of::<pyo3::types::PyType>()
                    && origin
                        .cast::<pyo3::types::PyType>()
                        .map(|t| t.name().map(|n| n == "tuple").unwrap_or(false))
                        .unwrap_or(false)
                {
                    let args = second_key.getattr("__args__")?;
                    let mut components = Vec::new();
                    for item in args.try_iter()? {
                        let item = item?;
                        let comp_type_obj = item.cast_exact::<PyType>().map_err(|_| {
                            PyTypeError::new_err(format!(
                                "All items in bundle filter tuple must be Component types, got {:?}",
                                item
                            ))
                        })?;
                        let comp_type = PyComponentType::try_from((comp_type_obj, py))?;
                        components.push(comp_type);
                    }
                    if components.is_empty() {
                        return Err(PyTypeError::new_err("Bundle filter tuple cannot be empty"));
                    }
                    Some(components)
                } else {
                    return Err(PyTypeError::new_err(format!(
                        "Second parameter to On must be a Component type or tuple[...], got {:?}",
                        second_key
                    )));
                }
            } else if let Ok(iter) = second_key.try_iter() {
                // Multiple components: On[Event, (A, B, C)] (actual tuple at runtime)
                let mut components = Vec::new();
                for item in iter {
                    let item = item?;
                    let comp_type_obj = item.cast_exact::<PyType>().map_err(|_| {
                        PyTypeError::new_err(format!(
                            "All items in bundle filter tuple must be Component types, got {:?}",
                            item
                        ))
                    })?;
                    let comp_type = PyComponentType::try_from((comp_type_obj, py))?;
                    components.push(comp_type);
                }
                if components.is_empty() {
                    return Err(PyTypeError::new_err("Bundle filter tuple cannot be empty"));
                }
                Some(components)
            } else {
                return Err(PyTypeError::new_err(format!(
                    "Second parameter to On must be a Component type or tuple of Component types, got {:?}",
                    second_key
                )));
            };

            Py::new(
                py,
                PyOnTypeParam {
                    event_type,
                    bundle_filter,
                },
            )
            .map(|obj| obj.into_any())
        } else {
            // On[E] - event type only, no bundle filter
            let event_type = if let Ok(event_type_obj) = key.cast_exact::<PyType>() {
                EventType::from_py_type(py, event_type_obj)?
            } else {
                return Err(PyTypeError::new_err(format!(
                    "Parameter to On must be an Event type, got {:?}",
                    key
                )));
            };

            Py::new(
                py,
                PyOnTypeParam {
                    event_type,
                    bundle_filter: None,
                },
            )
            .map(|obj| obj.into_any())
        }
    }

    /// Get the event data.
    pub fn event(&self, py: Python) -> Py<PyAny> {
        self.event_data.clone_ref(py)
    }

    /// Get the entity this event targets (for EntityEvents with 'entity' field).
    pub fn entity(&self) -> Option<PyEntity> {
        self.entity.map(PyEntity)
    }
}

/// Represents an event type in the observer system.
#[derive(Debug)]
pub enum EventType {
    /// Built-in component lifecycle events
    Add(PyComponentType),
    Insert(PyComponentType),
    Remove(PyComponentType),
    Replace(PyComponentType),
    Despawn(PyComponentType),

    /// Custom user-defined events (up to 20 supported)
    Custom(Py<PyType>),
}

impl Clone for EventType {
    fn clone(&self) -> Self {
        match self {
            EventType::Add(c) => EventType::Add(c.clone()),
            EventType::Insert(c) => EventType::Insert(c.clone()),
            EventType::Remove(c) => EventType::Remove(c.clone()),
            EventType::Replace(c) => EventType::Replace(c.clone()),
            EventType::Despawn(c) => EventType::Despawn(c.clone()),
            EventType::Custom(ty) => Python::attach(|py| EventType::Custom(ty.clone_ref(py))),
        }
    }
}

impl PartialEq for EventType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (EventType::Add(a), EventType::Add(b)) => a == b,
            (EventType::Insert(a), EventType::Insert(b)) => a == b,
            (EventType::Remove(a), EventType::Remove(b)) => a == b,
            (EventType::Replace(a), EventType::Replace(b)) => a == b,
            (EventType::Despawn(a), EventType::Despawn(b)) => a == b,
            (EventType::Custom(a), EventType::Custom(b)) => a.is(b),
            _ => false,
        }
    }
}

impl EventType {
    /// Extract event type from a Python type annotation.
    pub(crate) fn from_py_type(_py: Python, ty: &Bound<'_, PyType>) -> PyResult<Self> {
        // Check if it's a subclass of Event
        if ty.is_subclass_of::<PyEvent>()? {
            Ok(EventType::Custom(ty.clone().unbind()))
        } else {
            Err(PyTypeError::new_err(format!(
                "Expected Event subclass, got {}",
                ty.name()?
            )))
        }
    }

    /// Check if this event type matches a Python event instance.
    #[allow(dead_code)] // used by pybevy_control
    pub(crate) fn matches(&self, py: Python, event: &Bound<'_, PyAny>) -> bool {
        match self {
            EventType::Custom(expected_type) => {
                let event_type = event.get_type();
                event_type.is(&expected_type.bind(py))
            }
            // TODO: Implement lifecycle event matching
            _ => false,
        }
    }
}

/// Bundle filter for observers.
///
/// Observers with bundle filters only trigger if the target entity has
/// at least one of the components in the bundle (OR logic).
#[derive(Debug, Clone)]
pub struct BundleFilter {
    pub(crate) components: Vec<PyComponentType>,
}

impl BundleFilter {
    /// Check if an entity matches this bundle filter.
    /// Returns true if the entity has at least one of the components (OR logic).
    pub(crate) fn matches(
        &self,
        world: &bevy::ecs::world::World,
        entity: bevy::ecs::entity::Entity,
    ) -> bool {
        // Get entity reference
        let entity_ref = match world.get_entity(entity) {
            Ok(e) => e,
            Err(_) => return false, // Entity doesn't exist
        };

        // Check if entity has any of the required components (OR logic)
        self.components
            .iter()
            .any(|comp_type| entity_has_component(world, &entity_ref, comp_type))
    }
}

/// Helper function to check if an entity has a specific component type
fn entity_has_component(
    world: &bevy::ecs::world::World,
    entity: &bevy::ecs::world::EntityRef,
    comp_type: &PyComponentType,
) -> bool {
    match comp_type {
        PyComponentType::Custom(type_ptr) => {
            // For custom components, look up the ComponentId from the registry
            use crate::ecs::component_type::ComponentRegistry;

            if let Some(registry) = world.get_resource::<ComponentRegistry>() {
                if let Some(component_id) = registry.get(*type_ptr) {
                    // Check if entity has this component using the ComponentId
                    return entity.contains_id(component_id);
                }
            }
            // Component not registered or registry doesn't exist
            false
        }
        // For built-in components, use the generated entity_contains method
        _ => comp_type.entity_contains(entity),
    }
}

/// Public helper to check if an entity has a specific component type.
/// This is used by commands.rs to check for component existence before insertion.
pub(crate) fn entity_has_component_type(
    world: &bevy::ecs::world::World,
    entity: bevy::ecs::entity::Entity,
    comp_type: &PyComponentType,
) -> bool {
    match world.get_entity(entity) {
        Ok(entity_ref) => entity_has_component(world, &entity_ref, comp_type),
        Err(_) => false,
    }
}

/// Type parameter for On<E> or On<E, B> system parameters.
///
/// This is returned by On.__class_getitem__ when using On[EventType] or On[EventType, BundleType]
/// syntax in Python type annotations.
#[pyclass(name = "OnTypeParam", frozen)]
#[derive(Debug, Clone)]
pub struct PyOnTypeParam {
    pub(crate) event_type: EventType,
    pub(crate) bundle_filter: Option<Vec<PyComponentType>>,
}
