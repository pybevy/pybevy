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

use std::sync::{Arc, Mutex};

use bevy::ecs::{entity::Entity, schedule::ScheduleLabel, world::World};
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::PyType,
};

use pybevy_core::CustomComponentInfo;

use crate::ecs::{component::PyComponent, resource::PyResource};

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
    /// Type of the state enum (for validation)
    state_type: Py<PyType>,
}

#[pymethods]
impl PyState {
    /// Create a new State resource
    #[new]
    fn py_new(py: Python, initial_state: Py<PyAny>) -> PyResult<(Self, PyResource)> {
        let state_type = initial_state.bind(py).get_type().unbind();

        // Validate it's a registered state type
        Self::validate_state_type(py, &state_type)?;

        Ok((
            PyState {
                current: Arc::new(Mutex::new(initial_state)),
                state_type,
            },
            PyResource,
        ))
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
                    state_type,
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
    state_type: Py<PyType>,
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
    /// Queue a state transition
    ///
    /// The transition will be applied during the StateTransition schedule
    fn set(&self, py: Python, state: Py<PyAny>) -> PyResult<()> {
        // Validate state type matches
        let state_bound = state.bind(py);
        let provided_type = state_bound.get_type();

        if !provided_type.is(self.state_type.bind(py)) {
            return Err(PyTypeError::new_err(format!(
                "State type mismatch: expected {}, got {}",
                self.state_type.bind(py).name()?,
                provided_type.name()?
            )));
        }

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
                    state_type,
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

/// Bevy schedule labels for state transitions
/// These implement Bevy's ScheduleLabel trait to integrate with the schedule system

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ScheduleKind {
    Enter,
    Exit,
}

/// Rust-side schedule label for OnEnter/OnExit schedules
/// Uses hash-based approach for simplicity (as recommended in design doc)
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StateScheduleLabel {
    kind: ScheduleKind,
    state_hash: u64,
}

impl StateScheduleLabel {
    pub fn on_enter(state_hash: u64) -> Self {
        Self {
            kind: ScheduleKind::Enter,
            state_hash,
        }
    }

    pub fn on_exit(state_hash: u64) -> Self {
        Self {
            kind: ScheduleKind::Exit,
            state_hash,
        }
    }
}

/// Transition schedule label (for OnTransition)
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransitionScheduleLabel {
    exit_hash: u64,
    enter_hash: u64,
}

impl TransitionScheduleLabel {
    pub fn new(exit_hash: u64, enter_hash: u64) -> Self {
        Self {
            exit_hash,
            enter_hash,
        }
    }
}

/// Helper methods for Python schedule types to get their Bevy labels
impl PyOnEnterSchedule {
    pub fn to_bevy_label(&self, py: Python) -> PyResult<StateScheduleLabel> {
        let hash = self.state_value.bind(py).hash()? as u64;
        Ok(StateScheduleLabel::on_enter(hash))
    }
}

impl PyOnExitSchedule {
    pub fn to_bevy_label(&self, py: Python) -> PyResult<StateScheduleLabel> {
        let hash = self.state_value.bind(py).hash()? as u64;
        Ok(StateScheduleLabel::on_exit(hash))
    }
}

impl PyOnTransitionSchedule {
    pub fn to_bevy_label(&self, py: Python) -> PyResult<TransitionScheduleLabel> {
        let exit_hash = self.exited.bind(py).hash()? as u64;
        let enter_hash = self.entered.bind(py).hash()? as u64;
        Ok(TransitionScheduleLabel::new(exit_hash, enter_hash))
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
    fn new(py: Python, state: Py<PyAny>) -> PyResult<(Self, PyComponent)> {
        // Validate state type
        let state_type = state.bind(py).get_type().unbind();
        PyState::validate_state_type(py, &state_type)?;

        Ok((PyDespawnOnExit { state_value: state }, PyComponent))
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
    fn new(py: Python, state: Py<PyAny>) -> PyResult<(Self, PyComponent)> {
        // Validate state type
        let state_type = state.bind(py).get_type().unbind();
        PyState::validate_state_type(py, &state_type)?;

        Ok((PyDespawnOnEnter { state_value: state }, PyComponent))
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
    state_type_name: &str,
) -> PyResult<bool> {
    use bevy::ecs::schedule::Schedules;

    use crate::ecs::resource_type::PyResourceStorage;

    // Get the NextState resource for this state type
    // We need to find it by matching the Python type name stored in the resource
    let mut next_state_opt: Option<Py<PyNextState>> = None;
    let mut state_opt: Option<Py<PyState>> = None;

    // Scan resources to find State<T> and NextState<T> for this type
    {
        let pyresource = world.resource::<PyResourceStorage>();

        for (_, resource_py) in pyresource.resources.iter() {
            let resource = resource_py.bind(py);

            // Check if this is NextState for our type
            if let Ok(next_state) = resource.cast::<PyNextState>() {
                let type_name = next_state
                    .borrow()
                    .state_type
                    .bind(py)
                    .name()
                    .unwrap()
                    .to_string();
                if type_name == state_type_name {
                    next_state_opt = Some(
                        resource_py
                            .clone_ref(py)
                            .extract::<Py<PyNextState>>(py)
                            .unwrap(),
                    );
                }
            }

            // Check if this is State for our type
            if let Ok(state) = resource.cast::<PyState>() {
                let type_name = state
                    .borrow()
                    .state_type
                    .bind(py)
                    .name()
                    .unwrap()
                    .to_string();
                if type_name == state_type_name {
                    state_opt = Some(
                        resource_py
                            .clone_ref(py)
                            .extract::<Py<PyState>>(py)
                            .unwrap(),
                    );
                }
            }
        }
    }

    let next_state_py = match next_state_opt {
        Some(ns) => ns,
        None => return Ok(false), // No NextState resource for this type
    };

    let state_py = match state_opt {
        Some(s) => s,
        None => return Ok(false), // No State resource for this type
    };

    // Check if there's a pending transition and take it
    let next_state_borrow = next_state_py.bind(py).borrow();
    let pending_transition = next_state_borrow.take_pending();
    let initial_enter =
        pending_transition.is_none() && next_state_borrow.take_initial_enter_pending();
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
        let enter_label = StateScheduleLabel::on_enter(hash);
        let has_enter_schedule = world.resource::<Schedules>().contains(enter_label.clone());
        if has_enter_schedule {
            world.try_run_schedule(enter_label).ok();
        }
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

    // Run OnExit(old_state) schedule
    let exit_label = StateScheduleLabel::on_exit(old_hash);
    let has_exit_schedule = world.resource::<Schedules>().contains(exit_label.clone());
    if has_exit_schedule {
        world.try_run_schedule(exit_label).ok();
    }

    // Despawn entities with DespawnOnExit matching the old state
    despawn_matching_entities(py, world, "DespawnOnExit", &current_state);

    // Update State<T> resource
    {
        let state = state_py.bind(py).borrow();
        state.set_value(pending_transition.clone_ref(py));
    }

    // Run OnTransition(old_state -> new_state) schedule
    let transition_label = TransitionScheduleLabel::new(old_hash, new_hash);
    let has_transition_schedule = world
        .resource::<Schedules>()
        .contains(transition_label.clone());
    if has_transition_schedule {
        world.try_run_schedule(transition_label).ok();
    }

    // Run OnEnter(new_state) schedule
    let enter_label = StateScheduleLabel::on_enter(new_hash);
    let has_enter_schedule = world.resource::<Schedules>().contains(enter_label.clone());
    if has_enter_schedule {
        world.try_run_schedule(enter_label).ok();
    }

    // Despawn entities with DespawnOnEnter matching the new state
    despawn_matching_entities(py, world, "DespawnOnEnter", &pending_transition);

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
        let matches = sv
            .eq(target_state.bind(py))
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
/// State types are discovered dynamically by scanning the world's PyResourceStorage
/// for State<T> and NextState<T> resources.
pub fn apply_state_transitions(py: Python, world: &mut bevy::ecs::world::World) -> PyResult<()> {
    use std::{
        collections::HashSet,
        sync::atomic::{AtomicBool, Ordering},
    };

    use crate::ecs::resource_type::PyResourceStorage;

    // Guard against re-entrant calls (world.run_schedule for OnEnter/OnExit
    // can trigger the parent schedule to re-run)
    static PROCESSING: AtomicBool = AtomicBool::new(false);
    if PROCESSING.swap(true, Ordering::SeqCst) {
        return Ok(()); // Already processing, skip
    }
    struct ProcessingGuard;
    impl Drop for ProcessingGuard {
        fn drop(&mut self) {
            PROCESSING.store(false, Ordering::SeqCst);
        }
    }
    let _guard = ProcessingGuard;

    // Discover state type names by scanning PyResourceStorage for NextState<T> resources
    let mut state_type_names = HashSet::new();
    {
        let pyresource = world.resource::<PyResourceStorage>();
        for (_, resource_py) in pyresource.resources.iter() {
            let resource = resource_py.bind(py);
            // Check if this is NextState for any type
            if let Ok(next_state) = resource.cast::<PyNextState>() {
                let type_name = next_state
                    .borrow()
                    .state_type
                    .bind(py)
                    .name()
                    .unwrap()
                    .to_string();
                state_type_names.insert(type_name);
            }
        }
    }

    // Apply transitions for each discovered state type
    for type_name in state_type_names {
        apply_transition_for_state(py, world, &type_name)?;
    }

    Ok(())
}
