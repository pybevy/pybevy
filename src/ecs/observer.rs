use std::sync::Arc;

use pybevy_core::PyMessage;
use pyo3::{
    PyTraverseError, PyTypeInfo, PyVisit, exceptions::PyTypeError, prelude::*, types::PyType,
};
use smallvec::SmallVec;

use super::{
    PyEntity,
    component_type::{PyComponentType, clone_retained_classes, retain_custom_classes},
};

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
#[pyclass(name = "Event", module = "pybevy.ecs", subclass, skip_from_py_object)]
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
/// Use with On[Add, ComponentType] to observe when components are added.
#[pyclass(name = "Add", module = "pybevy.ecs", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAdd;

/// Marker class for component insertion lifecycle events.
/// Use with On[Insert, ComponentType] to observe when components are inserted.
#[pyclass(name = "Insert", module = "pybevy.ecs", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyInsert;

/// Marker class for component removal lifecycle events.
/// Use with On[Remove, ComponentType] to observe when components are removed.
#[pyclass(name = "Remove", module = "pybevy.ecs", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyRemove;

/// Marker class for component discard lifecycle events.
/// Use with On[Discard, ComponentType] to observe when a component value is
/// discarded because it is replaced, removed, or despawned. Fires before the
/// value is dropped, so observers can still read the original component data.
#[pyclass(name = "Discard", module = "pybevy.ecs", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyDiscard;

/// Marker class for entity despawn lifecycle events.
/// Use with On[Despawn, ComponentType] to observe when entities with the component are despawned.
#[pyclass(name = "Despawn", module = "pybevy.ecs", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyDespawn;

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
#[pyclass(name = "On", module = "pybevy.ecs", frozen)]
pub struct PyOn {
    pub(crate) event_data: Py<PyAny>,
    pub(crate) entity: Option<bevy::ecs::entity::Entity>,
}

#[pymethods]
impl PyOn {
    /// Report the triggered event retained by an escaped observer parameter.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.event_data)
    }

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
                return Err(PyTypeError::new_err(format!(
                    "On[Event, A, B] (or more) is not a valid form; use \
                     On[Event, tuple[A, B]] for multi-component filters. Got {} items.",
                    tuple.len()
                )));
            }

            let first_key = tuple.get_item(0)?;
            let second_key = tuple.get_item(1)?;

            // Check if first parameter is a lifecycle event marker
            let first_type_obj = first_key.cast_exact::<PyType>()?;

            // Check for lifecycle event markers
            if first_type_obj.is(PyAdd::type_object(py)) {
                // On[Add, Component] - component addition lifecycle event
                let comp_type = if let Ok(comp_type_obj) = second_key.cast_exact::<PyType>() {
                    PyComponentType::try_from((comp_type_obj, py))?
                } else {
                    return Err(PyTypeError::new_err(
                        "Second parameter to On[Add, ...] must be a Component type",
                    ));
                };
                return Py::new(py, PyOnTypeParam::new(py, EventType::Add(comp_type), None))
                    .map(|obj| obj.into_any());
            } else if first_type_obj.is(PyInsert::type_object(py)) {
                let comp_type = if let Ok(comp_type_obj) = second_key.cast_exact::<PyType>() {
                    PyComponentType::try_from((comp_type_obj, py))?
                } else {
                    return Err(PyTypeError::new_err(
                        "Second parameter to On[Insert, ...] must be a Component type",
                    ));
                };
                return Py::new(
                    py,
                    PyOnTypeParam::new(py, EventType::Insert(comp_type), None),
                )
                .map(|obj| obj.into_any());
            } else if first_type_obj.is(PyRemove::type_object(py)) {
                let comp_type = if let Ok(comp_type_obj) = second_key.cast_exact::<PyType>() {
                    PyComponentType::try_from((comp_type_obj, py))?
                } else {
                    return Err(PyTypeError::new_err(
                        "Second parameter to On[Remove, ...] must be a Component type",
                    ));
                };
                return Py::new(
                    py,
                    PyOnTypeParam::new(py, EventType::Remove(comp_type), None),
                )
                .map(|obj| obj.into_any());
            } else if first_type_obj.is(PyDiscard::type_object(py)) {
                let comp_type = if let Ok(comp_type_obj) = second_key.cast_exact::<PyType>() {
                    PyComponentType::try_from((comp_type_obj, py))?
                } else {
                    return Err(PyTypeError::new_err(
                        "Second parameter to On[Discard, ...] must be a Component type",
                    ));
                };
                return Py::new(
                    py,
                    PyOnTypeParam::new(py, EventType::Discard(comp_type), None),
                )
                .map(|obj| obj.into_any());
            } else if first_type_obj.is(PyDespawn::type_object(py)) {
                let comp_type = if let Ok(comp_type_obj) = second_key.cast_exact::<PyType>() {
                    PyComponentType::try_from((comp_type_obj, py))?
                } else {
                    return Err(PyTypeError::new_err(
                        "Second parameter to On[Despawn, ...] must be a Component type",
                    ));
                };
                return Py::new(
                    py,
                    PyOnTypeParam::new(py, EventType::Despawn(comp_type), None),
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

            Py::new(py, PyOnTypeParam::new(py, event_type, bundle_filter)).map(|obj| obj.into_any())
        } else {
            // On[E] - event type only, no bundle filter
            let event_type = if let Ok(event_type_obj) = key.cast_exact::<PyType>() {
                // Lifecycle markers (Add, Insert, Remove, Discard, Despawn) require
                // a component filter. The bare On[Add] / On[Despawn] form would
                // otherwise fall through to from_py_type and produce a confusing
                // "Expected Event subclass, got Despawn" error. Catch them with
                // an actionable message instead.
                let lifecycle_marker = if event_type_obj.is(PyAdd::type_object(py)) {
                    Some("Add")
                } else if event_type_obj.is(PyInsert::type_object(py)) {
                    Some("Insert")
                } else if event_type_obj.is(PyRemove::type_object(py)) {
                    Some("Remove")
                } else if event_type_obj.is(PyDiscard::type_object(py)) {
                    Some("Discard")
                } else if event_type_obj.is(PyDespawn::type_object(py)) {
                    Some("Despawn")
                } else {
                    None
                };
                if let Some(marker_name) = lifecycle_marker {
                    return Err(PyTypeError::new_err(format!(
                        "On[{marker_name}] requires a component filter: \
                         use On[{marker_name}, ComponentType] to observe \
                         {marker_name} events for a specific component."
                    )));
                }
                EventType::from_py_type(py, event_type_obj)?
            } else {
                return Err(PyTypeError::new_err(format!(
                    "Parameter to On must be an Event type, got {:?}",
                    key
                )));
            };

            Py::new(py, PyOnTypeParam::new(py, event_type, None)).map(|obj| obj.into_any())
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
    Discard(PyComponentType),
    Despawn(PyComponentType),

    /// Custom user-defined events (up to 20 supported)
    Custom(Py<PyType>),
}

impl EventType {
    /// Report a `Custom` event class to the cyclic GC; lifecycle variants hold
    /// component pointers whose classes are retained by `PyOnTypeParam`.
    pub(crate) fn traverse(&self, visit: &PyVisit<'_>) -> Result<(), PyTraverseError> {
        match self {
            EventType::Custom(class) => visit.call(class),
            _ => Ok(()),
        }
    }
}

impl Clone for EventType {
    fn clone(&self) -> Self {
        match self {
            EventType::Add(c) => EventType::Add(*c),
            EventType::Insert(c) => EventType::Insert(*c),
            EventType::Remove(c) => EventType::Remove(*c),
            EventType::Discard(c) => EventType::Discard(*c),
            EventType::Despawn(c) => EventType::Despawn(*c),
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
            (EventType::Discard(a), EventType::Discard(b)) => a == b,
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
        } else if ty.is_subclass_of::<PyMessage>()? {
            Err(PyTypeError::new_err(format!(
                "{} is a Message subclass; observers require Event subclasses",
                ty.name()?
            )))
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
                event_type.is(expected_type.bind(py))
            }
            // TODO: Implement lifecycle event matching
            _ => false,
        }
    }
}

/// Type parameter for On<E> or On<E, B> system parameters.
///
/// This is returned by On.__class_getitem__ when using On[EventType] or On[EventType, BundleType]
/// syntax in Python type annotations.
#[pyclass(name = "OnTypeParam", module = "pybevy.ecs", frozen, from_py_object)]
#[derive(Debug)]
pub struct PyOnTypeParam {
    pub(crate) event_type: EventType,
    pub(crate) bundle_filter: Option<Vec<PyComponentType>>,
    /// Strong references backing Python-owned component/resource pointers above.
    ///
    /// The param outlives the expression that built it, and nothing else is
    /// obliged to keep a `@component` class alive, so observer registration
    /// would otherwise dereference a freed type object.
    pub(crate) retained_types: SmallVec<[Arc<Py<PyType>>; 4]>,
}

/// Deep-clones `retained_types` so every holder owns independent increfs; a
/// shared handle visited by two traversing owners is a use-after-free.
impl Clone for PyOnTypeParam {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            event_type: self.event_type.clone(),
            bundle_filter: self.bundle_filter.clone(),
            retained_types: clone_retained_classes(py, &self.retained_types),
        })
    }
}

impl PyOnTypeParam {
    pub(crate) fn new(
        py: Python<'_>,
        event_type: EventType,
        bundle_filter: Option<Vec<PyComponentType>>,
    ) -> Self {
        let from_event = match &event_type {
            EventType::Add(component)
            | EventType::Insert(component)
            | EventType::Remove(component)
            | EventType::Discard(component)
            | EventType::Despawn(component) => Some(*component),
            EventType::Custom(_) => None,
        };
        let from_filter = bundle_filter.iter().flatten().copied();
        let retained = retain_custom_classes(py, from_event.into_iter().chain(from_filter));
        Self {
            event_type,
            bundle_filter,
            retained_types: retained,
        }
    }
}

#[pymethods]
impl PyOnTypeParam {
    /// Report held classes to the cyclic GC; see docs/safety.md.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.retained_types {
            visit.call(class.as_ref())?;
        }
        self.event_type.traverse(&visit)
    }
}
