//! State system implementation for PyBevy
//!
//! Provides finite state machine functionality using Python Enums as state types.
//! States are app-wide resources that can be used to model game flow (Menu, InGame, Paused, etc.)
//!
//! # Features
//! - State<S> resource - holds current state
//! - NextState<S> resource - queue state transitions
//! - OnEnter/OnExit schedules - lifecycle hooks
//! - in_state() run condition - conditional system execution
//! - DespawnOnExit/DespawnOnEnter - automatic entity cleanup

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use bevy::ecs::{entity::Entity, resource::Resource, world::World};
use pybevy_core::CustomComponentInfo;
use pybevy_ecs::shared::{
    schedule::{StateMachineId, StateScheduleKind, StateScheduleLabel, TransitionScheduleLabel},
    state_machine_registry::{
        StateMachineIdentityRegistry, StateMachineRegisterOutcome, StateTypeKey,
    },
    state_transition::{
        StateTransitionGate, StateTransitionPassGuard, StateTransitionPlan, StateTransitionStep,
    },
};
use pyo3::{
    PyTypeInfo,
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyDict, PyTuple, PyType},
};

use crate::ecs::{
    component::PyComponent,
    resource::PyResource,
    resource_type::{PyResourceStorage, register_custom_resource},
};

pub(crate) const STATE_RESOURCE_TYPE_CACHE_ATTR: &str = "__pybevy_state_resource_cache__";

/// Python decorator for marking an Enum as a state machine
///
/// # Example
/// ```python
/// from enum import Enum, auto
/// from pybevy import state
///
/// @state
/// class GameState(Enum):
///     MENU = auto()
///     IN_GAME = auto()
///     PAUSED = auto()
/// ```
#[pyfunction]
pub fn state(py: Python, cls: Bound<PyType>) -> PyResult<Py<PyType>> {
    // Validate: must be Enum subclass
    let enum_type = py.import("enum")?.getattr("Enum")?;
    if !cls.is_subclass(&enum_type)? {
        return Err(PyTypeError::new_err(
            "@state decorator can only be applied to Enum subclasses",
        ));
    }

    // Validate that enum has at least one variant
    let members_dict = cls.getattr("__members__")?;
    let values = members_dict.call_method0("values")?;
    let has_members = values.try_iter()?.next().is_some();

    if !has_members {
        return Err(PyValueError::new_err(
            "State enum must have at least one variant",
        ));
    }

    // Mark as state type - metadata will be read directly from the class when needed
    cls.setattr("__pybevy_state__", true)?;

    Ok(cls.unbind())
}

/// State<S> resource - holds the current state value
///
/// Access via Res[State[GameState]] in systems
#[pyclass(name = "State", frozen, extends = PyResource)]
pub struct PyState {
    /// Current state value (Python enum member) - uses internal mutability for transitions
    current: Arc<Mutex<Py<PyAny>>>,
}

#[pymethods]
impl PyState {
    #[classmethod]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        state_type: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyType>> {
        typed_state_resource_type(cls.py(), cls, state_type, StateResourceKind::Current)
    }

    /// Create a new State resource
    #[new]
    fn py_new(py: Python, initial_state: Py<PyAny>) -> PyResult<PyClassInitializer<Self>> {
        let state_type = initial_state.bind(py).get_type().unbind();

        // Validate it's a registered state type
        Self::validate_state_type(py, &state_type)?;

        Ok((
            PyState {
                current: Arc::new(Mutex::new(initial_state)),
            },
            PyResource,
        )
            .into())
    }

    /// Get the current state value
    fn get(&self, py: Python) -> Py<PyAny> {
        self.current.lock().unwrap().clone_ref(py)
    }

    /// Check if current state equals given state
    fn __eq__(&self, _py: Python, other: &Bound<PyAny>) -> PyResult<bool> {
        // CRITICAL: Cannot use PyO3 methods that require GIL (extract, bind, etc.)
        // because this is called from py.detach() context (app.rs:1192).

        // Check if comparing with the same State instance using raw pointer comparison
        let self_ptr = self as *const Self as *const ();
        let other_ptr = other.as_ptr() as *const ();

        // If comparing the exact same State object (self == other)
        if self_ptr == other_ptr {
            return Ok(true);
        }

        //Otherwise, we cannot safely extract or compare without GIL
        // Return false as a safe default (State comparisons should use 'is' not '==')
        Ok(false)
    }

    fn __repr__(&self, py: Python) -> PyResult<String> {
        let state_str = self.current.lock().unwrap().bind(py).repr()?.to_string();
        Ok(format!("State({})", state_str))
    }
}

impl PyState {
    /// Create a new State resource with given value
    pub fn new(py: Python, state_value: Py<PyAny>) -> PyResult<Py<Self>> {
        let state_type = state_value.bind(py).get_type().unbind();

        // Validate it's a registered state type
        Self::validate_state_type(py, &state_type)?;

        Py::new(
            py,
            (
                PyState {
                    current: Arc::new(Mutex::new(state_value)),
                },
                PyResource,
            ),
        )
    }

    /// Internal helper to get current state value
    pub fn current_value(&self, py: Python) -> Py<PyAny> {
        self.current.lock().unwrap().clone_ref(py)
    }

    /// Update the state value (used internally during transitions)
    pub fn set_value(&self, new_value: Py<PyAny>) {
        *self.current.lock().unwrap() = new_value;
    }

    fn validate_state_type(py: Python, state_type: &Py<PyType>) -> PyResult<()> {
        let type_bound = state_type.bind(py);

        if !type_bound.hasattr("__pybevy_state__")? {
            return Err(PyTypeError::new_err(format!(
                "Type '{}' is not a valid state type. Did you forget the @state decorator?",
                type_bound.name()?
            )));
        }

        Ok(())
    }
}

/// NextState<S> resource - queue for pending state transitions
///
/// Use ResMut[NextState[GameState]] to queue transitions
#[pyclass(name = "NextState", frozen, extends = PyResource)]
pub struct PyNextState {
    /// Internal state: Unchanged or Pending(value)
    inner: Arc<Mutex<NextStateInner>>,
    /// Type of the state enum
    state_type: Mutex<Py<PyType>>,
    /// Whether the initial OnEnter for the starting state still needs to fire.
    /// Set to true on creation, cleared after the first transition system run.
    /// This matches Bevy's behavior where OnEnter fires for the initial state.
    initial_enter_pending: Arc<Mutex<bool>>,
}

enum NextStateInner {
    Unchanged,
    Pending(Py<PyAny>),
}

#[pymethods]
impl PyNextState {
    #[classmethod]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        state_type: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyType>> {
        typed_state_resource_type(cls.py(), cls, state_type, StateResourceKind::Next)
    }

    /// Queue a state transition
    ///
    /// The transition will be applied during the StateTransition schedule
    fn set(&self, py: Python, state: Py<PyAny>) -> PyResult<()> {
        // Validate state type matches
        let state_bound = state.bind(py);
        let provided_type = state_bound.get_type();

        let state_type = self.state_type.lock().unwrap();
        if !provided_type.is(state_type.bind(py)) {
            return Err(PyTypeError::new_err(format!(
                "State type mismatch: expected {}, got {}",
                state_type.bind(py).name()?,
                provided_type.name()?
            )));
        }
        drop(state_type);

        *self.inner.lock().unwrap() = NextStateInner::Pending(state);
        Ok(())
    }

    /// Cancel any pending transition
    fn reset(&self) -> PyResult<()> {
        *self.inner.lock().unwrap() = NextStateInner::Unchanged;
        Ok(())
    }

    /// Check if a transition is pending
    fn is_pending(&self) -> bool {
        matches!(*self.inner.lock().unwrap(), NextStateInner::Pending(_))
    }

    /// Get the pending state without consuming it (for inspection)
    fn peek_pending(&self, py: Python) -> Option<Py<PyAny>> {
        match &*self.inner.lock().unwrap() {
            NextStateInner::Pending(state) => Some(state.clone_ref(py)),
            NextStateInner::Unchanged => None,
        }
    }

    fn __repr__(&self, py: Python) -> PyResult<String> {
        let inner = self.inner.lock().unwrap();
        match &*inner {
            NextStateInner::Unchanged => Ok("NextState(Unchanged)".to_string()),
            NextStateInner::Pending(state) => {
                let state_str = state.bind(py).repr()?.to_string();
                Ok(format!("NextState(Pending({}))", state_str))
            }
        }
    }
}

impl PyNextState {
    /// Create a new NextState resource (starts as Unchanged)
    pub fn new(py: Python, state_type: Py<PyType>) -> PyResult<Py<Self>> {
        PyState::validate_state_type(py, &state_type)?;

        Py::new(
            py,
            (
                PyNextState {
                    inner: Arc::new(Mutex::new(NextStateInner::Unchanged)),
                    state_type: Mutex::new(state_type),
                    initial_enter_pending: Arc::new(Mutex::new(true)),
                },
                PyResource,
            ),
        )
    }

    /// Take the pending state if any (used internally during transitions)
    pub fn take_pending(&self) -> Option<Py<PyAny>> {
        let mut inner = self.inner.lock().unwrap();
        match std::mem::replace(&mut *inner, NextStateInner::Unchanged) {
            NextStateInner::Pending(state) => Some(state),
            NextStateInner::Unchanged => None,
        }
    }

    /// Check and clear the initial enter pending flag.
    /// Returns true if the initial OnEnter still needs to fire.
    pub fn take_initial_enter_pending(&self) -> bool {
        let mut pending = self.initial_enter_pending.lock().unwrap();
        if *pending {
            *pending = false;
            true
        } else {
            false
        }
    }

    fn migrate_state_type(&self, py: Python<'_>, state_type: Py<PyType>) -> PyResult<()> {
        let pending = match &*self.inner.lock().unwrap() {
            NextStateInner::Pending(value) => Some(value.clone_ref(py)),
            NextStateInner::Unchanged => None,
        };
        let migrated = pending
            .map(|value| remap_state_value(py, &value, state_type.bind(py)))
            .transpose()?;

        *self.state_type.lock().unwrap() = state_type;
        if let Some(value) = migrated {
            *self.inner.lock().unwrap() = NextStateInner::Pending(value);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum StateResourceKind {
    Current,
    Next,
}

impl StateResourceKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Current => "State",
            Self::Next => "NextState",
        }
    }
}

fn state_type_qualified_name(state_type: &Bound<'_, PyType>) -> PyResult<String> {
    let module = state_type
        .getattr("__module__")?
        .extract::<String>()
        .unwrap_or_default();
    let qualname = state_type
        .getattr("__qualname__")
        .or_else(|_| state_type.getattr("__name__"))?
        .extract::<String>()?;
    Ok(if module.is_empty() {
        qualname
    } else {
        format!("{module}.{qualname}")
    })
}

fn typed_state_resource_type(
    py: Python<'_>,
    wrapper_type: &Bound<'_, PyType>,
    state_type: &Bound<'_, PyAny>,
    kind: StateResourceKind,
) -> PyResult<Py<PyType>> {
    let state_type = state_type.cast::<PyType>().map_err(|_| {
        PyTypeError::new_err(format!("{}[...] requires a @state Enum type", kind.name()))
    })?;
    PyState::validate_state_type(py, &state_type.clone().unbind())?;

    let cache = wrapper_type
        .getattr(STATE_RESOURCE_TYPE_CACHE_ATTR)?
        .cast_into::<PyDict>()?;
    if let Some(cached) = cache.get_item(state_type)? {
        return Ok(cached.cast_into::<PyType>()?.unbind());
    }

    let state_name = state_type_qualified_name(state_type)?;
    let descriptor_name = format!("{}[{state_name}]", kind.name());
    let namespace = PyDict::new(py);
    namespace.set_item("__module__", "pybevy.ecs")?;
    namespace.set_item("__qualname__", &descriptor_name)?;
    namespace.set_item("__pybevy_resource_decorated__", true)?;
    namespace.set_item("__pybevy_state_type__", state_type)?;
    namespace.set_item("__pybevy_state_kind__", kind.name())?;
    let bases = PyTuple::new(py, [PyResource::type_object(py)])?;
    let descriptor = py
        .get_type::<PyType>()
        .call1((&descriptor_name, bases, namespace))?
        .cast_into::<PyType>()?;
    // set_default_with_result is the canonicalization point: concurrent cache
    // misses may build short-lived candidates, but every caller receives the
    // single descriptor stored for this exact state class.
    let (_, canonical) = cache.set_default_with_result(state_type, &descriptor)?;
    Ok(canonical.cast_into::<PyType>()?.unbind())
}

struct PyStateMachineEntry {
    state: Py<PyState>,
    next_state: Py<PyNextState>,
}

#[derive(Resource, Default)]
pub(crate) struct PyStateMachineRegistry {
    order: Vec<StateMachineId>,
    entries: HashMap<StateMachineId, PyStateMachineEntry>,
    identities: StateMachineIdentityRegistry,
    transition_gate: StateTransitionGate,
}

impl PyStateMachineRegistry {
    fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_ambiguous(&self) -> bool {
        self.len() > 1
    }

    fn snapshots(&self, py: Python<'_>) -> Vec<(StateMachineId, Py<PyState>, Py<PyNextState>)> {
        self.order
            .iter()
            .filter_map(|machine_id| {
                self.entries.get(machine_id).map(|entry| {
                    (
                        *machine_id,
                        entry.state.clone_ref(py),
                        entry.next_state.clone_ref(py),
                    )
                })
            })
            .collect()
    }

    pub(crate) fn canonical_id(&self, type_key: StateTypeKey) -> Option<StateMachineId> {
        self.identities.get(type_key)
    }

    fn begin_transition_pass(&self) -> Option<StateTransitionPassGuard> {
        self.transition_gate.try_enter()
    }
}

fn remap_state_value(
    py: Python<'_>,
    value: &Py<PyAny>,
    state_type: &Bound<'_, PyType>,
) -> PyResult<Py<PyAny>> {
    let member_name = value.bind(py).getattr("name")?.extract::<String>()?;
    Ok(state_type.get_item(member_name)?.unbind())
}

fn state_values_match(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<bool> {
    if left.eq(right)? {
        return Ok(true);
    }

    let left_type = left.get_type();
    let right_type = right.get_type();
    if state_type_qualified_name(&left_type)? != state_type_qualified_name(&right_type)? {
        return Ok(false);
    }

    let left_name = left.getattr("name")?.extract::<String>()?;
    let right_name = right.getattr("name")?.extract::<String>()?;
    Ok(left_name == right_name)
}

pub(crate) fn untyped_state_resource_name(
    resource_type: &Bound<'_, PyType>,
) -> Option<&'static str> {
    let py = resource_type.py();
    if resource_type.is(PyState::type_object(py)) {
        Some("State")
    } else if resource_type.is(PyNextState::type_object(py)) {
        Some("NextState")
    } else {
        None
    }
}

pub(crate) fn insert_state_machine_resources(
    py: Python<'_>,
    world: &mut World,
    state_type: Py<PyType>,
    state: Py<PyState>,
    next_state: Py<PyNextState>,
) -> PyResult<()> {
    let state_type_bound = state_type.bind(py);
    let qualified_name = state_type_qualified_name(state_type_bound)?;
    if !world.contains_resource::<PyStateMachineRegistry>() {
        world.insert_resource(PyStateMachineRegistry::default());
    }
    let machine_id = world
        .resource_mut::<PyStateMachineRegistry>()
        .identities
        .register(state_type_key(state_type_bound), &qualified_name)
        .machine_id();

    let machine_count = {
        let mut registry = world.resource_mut::<PyStateMachineRegistry>();
        if !registry.entries.contains_key(&machine_id) {
            registry.order.push(machine_id);
        }
        registry.entries.insert(
            machine_id,
            PyStateMachineEntry {
                state: state.clone_ref(py),
                next_state: next_state.clone_ref(py),
            },
        );
        registry.len()
    };

    bind_state_machine_resource_channels(
        py,
        world,
        state_type_bound,
        state,
        next_state,
        machine_count,
    )
}

fn bind_state_machine_resource_channels(
    py: Python<'_>,
    world: &mut World,
    state_type: &Bound<'_, PyType>,
    state: Py<PyState>,
    next_state: Py<PyNextState>,
    machine_count: usize,
) -> PyResult<()> {
    let typed_state = typed_state_resource_type(
        py,
        &PyState::type_object(py),
        state_type.as_any(),
        StateResourceKind::Current,
    )?;
    let typed_next = typed_state_resource_type(
        py,
        &PyNextState::type_object(py),
        state_type.as_any(),
        StateResourceKind::Next,
    )?;

    let typed_state_bound = typed_state.bind(py);
    let typed_next_bound = typed_next.bind(py);
    let typed_state_id = register_custom_resource(
        world,
        typed_state_bound.as_type_ptr(),
        typed_state_bound.name()?.to_string(),
    );
    let typed_next_id = register_custom_resource(
        world,
        typed_next_bound.as_type_ptr(),
        typed_next_bound.name()?.to_string(),
    );
    let legacy_state_type = PyState::type_object(py);
    let legacy_next_type = PyNextState::type_object(py);
    let legacy_state_id = register_custom_resource(
        world,
        legacy_state_type.as_type_ptr(),
        legacy_state_type.name()?.to_string(),
    );
    let legacy_next_id = register_custom_resource(
        world,
        legacy_next_type.as_type_ptr(),
        legacy_next_type.name()?.to_string(),
    );

    if !world.contains_resource::<PyResourceStorage>() {
        world.insert_resource(PyResourceStorage::default());
    }
    let mut storage = world.resource_mut::<PyResourceStorage>();
    storage
        .resources
        .insert(typed_state_id, state.clone_ref(py).into_any());
    storage
        .resources
        .insert(typed_next_id, next_state.clone_ref(py).into_any());
    if machine_count == 1 {
        storage.resources.insert(legacy_state_id, state.into_any());
        storage
            .resources
            .insert(legacy_next_id, next_state.into_any());
    } else {
        storage.resources.remove(&legacy_state_id);
        storage.resources.remove(&legacy_next_id);
    }

    Ok(())
}

/// Attach a state class produced by hot reload to its stable logical machine.
///
/// Full reload supplies `reset = true`, rebuilding the state from its entrypoint
/// declaration after custom resource cleanup. Partial reload migrates retained
/// current/pending enum members by member name and preserves transition flags.
pub(crate) fn register_reloaded_state_machine(
    py: Python<'_>,
    world: &mut World,
    state_type: Py<PyType>,
    initial_state: Py<PyAny>,
    reset: bool,
) -> PyResult<()> {
    if reset {
        let state = PyState::new(py, initial_state)?;
        let next_state = PyNextState::new(py, state_type.clone_ref(py))?;
        return insert_state_machine_resources(py, world, state_type, state, next_state);
    }

    let state_type_bound = state_type.bind(py);
    let qualified_name = state_type_qualified_name(state_type_bound)?;
    if !world.contains_resource::<PyStateMachineRegistry>() {
        world.insert_resource(PyStateMachineRegistry::default());
    }
    let outcome = world
        .resource_mut::<PyStateMachineRegistry>()
        .identities
        .register(state_type_key(state_type_bound), &qualified_name);
    let machine_id = outcome.machine_id();

    if matches!(outcome, StateMachineRegisterOutcome::Registered(_)) {
        let state = PyState::new(py, initial_state)?;
        let next_state = PyNextState::new(py, state_type.clone_ref(py))?;
        return insert_state_machine_resources(py, world, state_type, state, next_state);
    }

    let (state, next_state, machine_count) = {
        let registry = world.resource::<PyStateMachineRegistry>();
        let entry = registry.entries.get(&machine_id).ok_or_else(|| {
            PyRuntimeError::new_err("state identity exists without a machine entry")
        })?;
        (
            entry.state.clone_ref(py),
            entry.next_state.clone_ref(py),
            registry.len(),
        )
    };

    let current = state.bind(py).borrow().current_value(py);
    let migrated_current = remap_state_value(py, &current, state_type_bound)?;
    next_state
        .bind(py)
        .borrow()
        .migrate_state_type(py, state_type.clone_ref(py))?;
    state.bind(py).borrow().set_value(migrated_current);

    bind_state_machine_resource_channels(
        py,
        world,
        state_type_bound,
        state,
        next_state,
        machine_count,
    )
}

/// Schedule label for systems that run when entering a state
#[pyclass(name = "OnEnterSchedule", frozen)]
pub struct PyOnEnterSchedule {
    state_value: Py<PyAny>,
}

#[pymethods]
impl PyOnEnterSchedule {
    fn __repr__(&self, py: Python) -> PyResult<String> {
        let state_str = self.state_value.bind(py).repr()?.to_string();
        Ok(format!("OnEnter({})", state_str))
    }

    fn __hash__(&self, py: Python) -> PyResult<u64> {
        Ok(self.state_value.bind(py).hash()? as u64)
    }

    fn __eq__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<bool> {
        // CRITICAL: Use 'is' semantics (pointer equality) instead of Python's __eq__
        // to avoid deadlock when called from systems in py.detach() context (app.rs:1192)
        if let Ok(other_schedule) = other.extract::<PyRef<Self>>() {
            Ok(self
                .state_value
                .bind(py)
                .is(other_schedule.state_value.bind(py)))
        } else {
            Ok(false)
        }
    }
}

/// Schedule label for systems that run when exiting a state
#[pyclass(name = "OnExitSchedule", frozen)]
pub struct PyOnExitSchedule {
    state_value: Py<PyAny>,
}

#[pymethods]
impl PyOnExitSchedule {
    fn __repr__(&self, py: Python) -> PyResult<String> {
        let state_str = self.state_value.bind(py).repr()?.to_string();
        Ok(format!("OnExit({})", state_str))
    }

    fn __hash__(&self, py: Python) -> PyResult<u64> {
        Ok(self.state_value.bind(py).hash()? as u64)
    }

    fn __eq__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<bool> {
        // CRITICAL: Use 'is' semantics (pointer equality) instead of Python's __eq__
        // to avoid deadlock when called from systems in py.detach() context (app.rs:1192)
        if let Ok(other_schedule) = other.extract::<PyRef<Self>>() {
            Ok(self
                .state_value
                .bind(py)
                .is(other_schedule.state_value.bind(py)))
        } else {
            Ok(false)
        }
    }
}

/// Schedule label for systems that run during state transitions
#[pyclass(name = "OnTransitionSchedule", frozen)]
pub struct PyOnTransitionSchedule {
    exited: Py<PyAny>,
    entered: Py<PyAny>,
}

#[pymethods]
impl PyOnTransitionSchedule {
    fn __repr__(&self, py: Python) -> PyResult<String> {
        let exited_str = self.exited.bind(py).repr()?.to_string();
        let entered_str = self.entered.bind(py).repr()?.to_string();
        Ok(format!("OnTransition({} -> {})", exited_str, entered_str))
    }

    fn __hash__(&self, py: Python) -> PyResult<u64> {
        // Combine hashes of both states
        let hash1 = self.exited.bind(py).hash()? as u64;
        let hash2 = self.entered.bind(py).hash()? as u64;
        Ok(hash1.wrapping_mul(31).wrapping_add(hash2))
    }

    fn __eq__(&self, py: Python, other: &Bound<PyAny>) -> PyResult<bool> {
        // CRITICAL: Use 'is' semantics (pointer equality) instead of Python's __eq__
        // to avoid deadlock when called from systems in py.detach() context (app.rs:1192)
        if let Ok(other_schedule) = other.extract::<PyRef<Self>>() {
            let exited_eq = self.exited.bind(py).is(other_schedule.exited.bind(py));
            let entered_eq = self.entered.bind(py).is(other_schedule.entered.bind(py));
            Ok(exited_eq && entered_eq)
        } else {
            Ok(false)
        }
    }
}

fn state_type_key(state_type: &Bound<'_, PyType>) -> StateTypeKey {
    state_type.as_type_ptr() as usize
}

fn state_machine_id(state_type: &Bound<'_, PyType>) -> StateMachineId {
    StateMachineId::new(state_type_key(state_type))
}

pub(crate) fn canonicalize_state_schedule_label(
    world: &World,
    label: StateScheduleLabel,
) -> StateScheduleLabel {
    let machine_id = world
        .get_resource::<PyStateMachineRegistry>()
        .and_then(|registry| registry.canonical_id(label.machine_id().get()))
        .unwrap_or(label.machine_id());
    match label.kind() {
        StateScheduleKind::Enter => StateScheduleLabel::on_enter(machine_id, label.state_hash()),
        StateScheduleKind::Exit => StateScheduleLabel::on_exit(machine_id, label.state_hash()),
    }
}

pub(crate) fn canonicalize_transition_schedule_label(
    world: &World,
    label: TransitionScheduleLabel,
) -> TransitionScheduleLabel {
    let machine_id = world
        .get_resource::<PyStateMachineRegistry>()
        .and_then(|registry| registry.canonical_id(label.machine_id().get()))
        .unwrap_or(label.machine_id());
    TransitionScheduleLabel::new(machine_id, label.exit_hash(), label.enter_hash())
}

/// Helper methods for Python schedule types to get their Bevy labels
impl PyOnEnterSchedule {
    pub fn to_bevy_label(&self, py: Python) -> PyResult<StateScheduleLabel> {
        let state_value = self.state_value.bind(py);
        let machine_id = state_machine_id(&state_value.get_type());
        let hash = state_value.hash()? as u64;
        Ok(StateScheduleLabel::on_enter(machine_id, hash))
    }
}

impl PyOnExitSchedule {
    pub fn to_bevy_label(&self, py: Python) -> PyResult<StateScheduleLabel> {
        let state_value = self.state_value.bind(py);
        let machine_id = state_machine_id(&state_value.get_type());
        let hash = state_value.hash()? as u64;
        Ok(StateScheduleLabel::on_exit(machine_id, hash))
    }
}

impl PyOnTransitionSchedule {
    pub fn to_bevy_label(&self, py: Python) -> PyResult<TransitionScheduleLabel> {
        let exited = self.exited.bind(py);
        let machine_id = state_machine_id(&exited.get_type());
        let exit_hash = exited.hash()? as u64;
        let enter_hash = self.entered.bind(py).hash()? as u64;
        Ok(TransitionScheduleLabel::new(
            machine_id, exit_hash, enter_hash,
        ))
    }
}

/// Create an OnEnter schedule label
///
/// # Example
/// ```python
/// app.add_systems(OnEnter(GameState.MENU), setup_menu)
/// ```
#[pyfunction]
#[pyo3(name = "OnEnter")]
pub fn on_enter(py: Python, state: Py<PyAny>) -> PyResult<Py<PyOnEnterSchedule>> {
    // Validate state type
    let state_type = state.bind(py).get_type().unbind();
    PyState::validate_state_type(py, &state_type)?;

    Py::new(py, PyOnEnterSchedule { state_value: state })
}

/// Create an OnExit schedule label
///
/// # Example
/// ```python
/// app.add_systems(OnExit(GameState.MENU), cleanup_menu)
/// ```
#[pyfunction]
#[pyo3(name = "OnExit")]
pub fn on_exit(py: Python, state: Py<PyAny>) -> PyResult<Py<PyOnExitSchedule>> {
    // Validate state type
    let state_type = state.bind(py).get_type().unbind();
    PyState::validate_state_type(py, &state_type)?;

    Py::new(py, PyOnExitSchedule { state_value: state })
}

/// Create an OnTransition schedule label
///
/// # Example
/// ```python
/// app.add_systems(OnTransition(GameState.MENU, GameState.IN_GAME), transition_effect)
/// ```
#[pyfunction]
#[pyo3(name = "OnTransition")]
pub fn on_transition(
    py: Python,
    exited: Py<PyAny>,
    entered: Py<PyAny>,
) -> PyResult<Py<PyOnTransitionSchedule>> {
    // Validate both states are same type
    let exited_type = exited.bind(py).get_type();
    let entered_type = entered.bind(py).get_type();

    if !exited_type.is(&entered_type) {
        return Err(PyTypeError::new_err(
            "OnTransition requires both states to be the same type",
        ));
    }

    PyState::validate_state_type(py, &exited_type.unbind())?;

    Py::new(py, PyOnTransitionSchedule { exited, entered })
}

/// Component for entities that should despawn when exiting a state
///
/// # Example
/// ```python
/// commands.spawn((
///     Player(),
///     DespawnOnExit(GameState.IN_GAME)
/// ))
/// ```
#[pyclass(name = "DespawnOnExit", frozen, extends = PyComponent)]
pub struct PyDespawnOnExit {
    state_value: Py<PyAny>,
}

#[pymethods]
impl PyDespawnOnExit {
    #[new]
    fn new(py: Python, state: Py<PyAny>) -> PyResult<PyClassInitializer<Self>> {
        // Validate state type
        let state_type = state.bind(py).get_type().unbind();
        PyState::validate_state_type(py, &state_type)?;

        Ok((PyDespawnOnExit { state_value: state }, PyComponent).into())
    }

    fn __repr__(&self, py: Python) -> PyResult<String> {
        let state_str = self.state_value.bind(py).repr()?.to_string();
        Ok(format!("DespawnOnExit({})", state_str))
    }

    /// Get the state value this component is associated with
    fn state_value(&self, py: Python) -> Py<PyAny> {
        self.state_value.clone_ref(py)
    }
}

/// Component for entities that should despawn when entering a state
///
/// # Example
/// ```python
/// commands.spawn((
///     MenuUI(),
///     DespawnOnEnter(GameState.IN_GAME)
/// ))
/// ```
#[pyclass(name = "DespawnOnEnter", frozen, extends = PyComponent)]
pub struct PyDespawnOnEnter {
    state_value: Py<PyAny>,
}

#[pymethods]
impl PyDespawnOnEnter {
    #[new]
    fn new(py: Python, state: Py<PyAny>) -> PyResult<PyClassInitializer<Self>> {
        // Validate state type
        let state_type = state.bind(py).get_type().unbind();
        PyState::validate_state_type(py, &state_type)?;

        Ok((PyDespawnOnEnter { state_value: state }, PyComponent).into())
    }

    fn __repr__(&self, py: Python) -> PyResult<String> {
        let state_str = self.state_value.bind(py).repr()?.to_string();
        Ok(format!("DespawnOnEnter({})", state_str))
    }

    /// Get the state value this component is associated with
    fn state_value(&self, py: Python) -> Py<PyAny> {
        self.state_value.clone_ref(py)
    }
}

/// Run condition that checks if current state matches given state
///
/// # Example
/// ```python
/// app.add_systems(Update, menu_system.run_if(in_state(GameState.MENU)))
/// ```
#[pyfunction]
pub fn in_state(py: Python, state: Py<PyAny>) -> PyResult<Py<PyAny>> {
    // Validate that the state is from a @state decorated enum
    let state_type = state.bind(py).get_type().unbind();
    PyState::validate_state_type(py, &state_type)?;

    // Create a Python function that checks if current State == state
    // The function will have signature: (current: Res[State]) -> bool
    use std::ffi::CString;

    use pyo3::types::PyDict;

    // Create globals dict with required imports
    let globals = PyDict::new(py);
    let ecs_module = py.import("pybevy.ecs")?;
    globals.set_item("Res", ecs_module.getattr("Res")?)?;
    globals.set_item("State", ecs_module.getattr("State")?)?;

    let locals = PyDict::new(py);
    locals.set_item("target_state", state)?;

    let code = CString::new(
        r#"
def _make_in_state_condition(target):
    """Factory that creates a condition checking for a specific state."""
    def in_state_condition(current: Res[State]) -> bool:
        """Check if current State matches the target state."""
        return current.get() == target
    # Set a meaningful name for debugging
    in_state_condition.__name__ = f"in_state({target})"
    return in_state_condition

result = _make_in_state_condition(target_state)
"#,
    )?;

    py.run(&code, Some(&globals), Some(&locals))?;
    let condition = locals
        .get_item("result")?
        .ok_or_else(|| PyRuntimeError::new_err("Failed to create in_state condition"))?;

    Ok(condition.unbind())
}

/// Apply pending state transitions for all registered state types
/// System function that processes state transitions for a specific state type
///
/// This is called automatically in the StateTransition schedule to handle
/// NextState -> State updates and trigger OnExit/OnEnter schedules.
///
/// Returns true if a transition occurred, false otherwise.
fn apply_transition_for_state(
    py: Python,
    world: &mut World,
    machine_id: StateMachineId,
    state_py: Py<PyState>,
    next_state_py: Py<PyNextState>,
) -> PyResult<bool> {
    use bevy::ecs::schedule::Schedules;

    // Check if there's a pending transition and take it
    let next_state_borrow = next_state_py.bind(py).borrow();
    let pending_transition = next_state_borrow.take_pending();
    let initial_enter_pending = next_state_borrow.take_initial_enter_pending();
    let initial_enter = pending_transition.is_none() && initial_enter_pending;
    drop(next_state_borrow); // Drop borrow

    // If this is the initial enter (no explicit transition queued), just fire OnEnter
    // for the current state. This matches Bevy's behavior where OnEnter fires for
    // the state set via insert_state() on the first frame.
    if initial_enter {
        let current_state = {
            let state = state_py.bind(py).borrow();
            state.current_value(py)
        };
        let hash = current_state.bind(py).hash()? as u64;
        StateTransitionPlan::initial_enter().run(|step| {
            match step {
                StateTransitionStep::RunEnter => {
                    let enter_label = StateScheduleLabel::on_enter(machine_id, hash);
                    if world.resource::<Schedules>().contains(enter_label.clone()) {
                        world.try_run_schedule(enter_label).ok();
                    }
                }
                StateTransitionStep::CleanupEntered => {
                    despawn_matching_entities(py, world, "DespawnOnEnter", &current_state);
                }
                _ => unreachable!("invalid step in initial-enter plan"),
            }
            Ok::<(), PyErr>(())
        })?;
        return Ok(true);
    }

    let pending_transition = match pending_transition {
        Some(new_state) => new_state,
        None => return Ok(false), // No pending transition
    };

    // Get current state
    let current_state = {
        let state = state_py.bind(py).borrow();
        state.current_value(py)
    };
    // Get hash values for schedule lookup
    let old_hash = current_state.bind(py).hash()? as u64;
    let new_hash = pending_transition.bind(py).hash()? as u64;

    StateTransitionPlan::change().run(|step| {
        match step {
            StateTransitionStep::CommitNew => {
                let state = state_py.bind(py).borrow();
                state.set_value(pending_transition.clone_ref(py));
            }
            StateTransitionStep::RunExit => {
                let exit_label = StateScheduleLabel::on_exit(machine_id, old_hash);
                if world.resource::<Schedules>().contains(exit_label.clone()) {
                    world.try_run_schedule(exit_label).ok();
                }
            }
            StateTransitionStep::CleanupExited => {
                despawn_matching_entities(py, world, "DespawnOnExit", &current_state);
            }
            StateTransitionStep::RunTransition => {
                let transition_label = TransitionScheduleLabel::new(machine_id, old_hash, new_hash);
                if world
                    .resource::<Schedules>()
                    .contains(transition_label.clone())
                {
                    world.try_run_schedule(transition_label).ok();
                }
            }
            StateTransitionStep::RunEnter => {
                let enter_label = StateScheduleLabel::on_enter(machine_id, new_hash);
                if world.resource::<Schedules>().contains(enter_label.clone()) {
                    world.try_run_schedule(enter_label).ok();
                }
            }
            StateTransitionStep::CleanupEntered => {
                despawn_matching_entities(py, world, "DespawnOnEnter", &pending_transition);
            }
        }
        Ok::<(), PyErr>(())
    })?;

    Ok(true)
}

/// Despawn entities whose `component_name` component has a `state_value()`
/// matching `target_state`. Used during state transitions for DespawnOnExit
/// and DespawnOnEnter.
fn despawn_matching_entities(
    py: Python,
    world: &mut World,
    component_name: &str,
    target_state: &Py<PyAny>,
) {
    let comp_id = match world.get_resource::<CustomComponentInfo>() {
        Some(ci) => ci
            .iter()
            .find(|(_, entry)| entry.name == component_name && entry.is_pyobject_storage)
            .map(|(id, _)| id),
        None => return,
    };

    let Some(comp_id) = comp_id else {
        return;
    };

    let entities: Vec<Entity> = world.query::<Entity>().iter(world).collect();

    for entity in entities {
        let Ok(entity_ref) = world.get_entity(entity) else {
            continue;
        };
        let Ok(ptr) = entity_ref.get_by_id(comp_id) else {
            continue;
        };
        // SAFETY: is_pyobject_storage verified in lookup above, so raw data is Py<PyAny>
        let py_obj: &Py<PyAny> = unsafe { &*(ptr.as_ptr() as *const Py<PyAny>) };
        let sv = py_obj
            .bind(py)
            .call_method0("state_value")
            .expect("DespawnOnExit/DespawnOnEnter component missing state_value() method");
        let matches = state_values_match(&sv, target_state.bind(py))
            .expect("state_value() comparison failed");
        if matches {
            world.despawn(entity);
        }
    }
}

/// System that checks all registered state types and applies pending transitions.
///
/// This is automatically added to the PreUpdate schedule when
/// init_state() or insert_state() is called. It runs OnExit/OnTransition/OnEnter
/// schedules when a pending transition is found.
///
/// State machines are snapshotted from the exact-type registry before any
/// transition schedule runs, so callbacks never execute while it is borrowed.
pub fn apply_state_transitions(py: Python, world: &mut bevy::ecs::world::World) -> PyResult<()> {
    // Guard only this World's transition pass. A process-global guard would
    // incorrectly suppress another App running on a different thread.
    let Some((guard, machines)) =
        world
            .get_resource::<PyStateMachineRegistry>()
            .and_then(|registry| {
                registry
                    .begin_transition_pass()
                    .map(|guard| (guard, registry.snapshots(py)))
            })
    else {
        return Ok(());
    };
    let _guard = guard;

    for (machine_id, state, next_state) in machines {
        apply_transition_for_state(py, world, machine_id, state, next_state)?;
    }

    Ok(())
}
