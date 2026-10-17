use std::{
    ptr::NonNull,
    sync::{Arc, Mutex, Weak},
};

use bevy::ecs::{
    change_detection::Tick, system::Commands, world::unsafe_world_cell::UnsafeWorldCell,
};
use pybevy_core::public_error::{
    PARAM_SET_EXPECTS_TUPLE, PARAM_SET_MEMBER_COUNT, PARAM_SET_OUTSIDE_SYSTEM,
    param_set_member_unavailable,
};
use pybevy_ecs::shared::parity_trace::ParityRunHandle;
use pyo3::{
    PyTraverseError, PyTypeInfo, PyVisit,
    exceptions::{PyRuntimeError, PyTypeError},
    prelude::*,
    types::{PyAny, PyTuple, PyType},
};
use smallvec::SmallVec;

use super::{
    commands::CommandErrorSink, dynamic_system::build_run_args, messages::CursorStorage,
    query::query_runtime::CachedQuery, system::SystemParam, view::cached_view::CachedPyView,
};
use crate::ecs::helpers::validity_guard::ValidityFlag;

/// Most members a `ParamSet` accepts, matching Bevy's `p0()` to `p7()`.
pub(crate) const MAX_PARAM_SET_MEMBERS: usize = 8;

/// The `ParamSet[tuple[...]]` annotation: its member annotations, lowered by
/// the ordinary system parameter parser.
#[pyclass(name = "_ParamSetParam", module = "pybevy.ecs", frozen, from_py_object)]
#[derive(Debug)]
pub(crate) struct PyParamSetParam {
    members: Vec<Py<PyAny>>,
}

impl Clone for PyParamSetParam {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            members: self
                .members
                .iter()
                .map(|member| member.clone_ref(py))
                .collect(),
        })
    }
}

impl PyParamSetParam {
    pub(crate) fn members(&self) -> &[Py<PyAny>] {
        &self.members
    }
}

#[pymethods]
impl PyParamSetParam {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for member in &self.members {
            visit.call(member)?;
        }
        Ok(())
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let members = self
            .members
            .iter()
            .map(|member| member.bind(py).repr().map(|value| value.to_string()))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(format!("ParamSet[tuple[{}]]", members.join(", ")))
    }
}

/// Where one member's run state lives in the system's scheduled state.
pub(crate) struct ParamSetMember {
    pub(crate) query_cache: Option<NonNull<CachedQuery>>,
    pub(crate) view_cache: Option<NonNull<Result<Arc<CachedPyView>, Arc<str>>>>,
    pub(crate) message_cursor: Option<CursorStorage>,
}

/// System parameter whose members may conflict with each other; `pN()`
/// borrows member N and ends the borrow handed out by the previous call.
#[pyclass(name = "ParamSet", module = "pybevy.ecs", frozen)]
pub(crate) struct PyParamSet {
    /// The member descriptors, upgraded per borrow. Weak, so an escaped set
    /// keeps no scene class alive once its system is gone.
    descriptors: Weak<Vec<SystemParam>>,
    members: Vec<ParamSetMember>,
    world: UnsafeWorldCell<'static>,
    validity: ValidityFlag,
    active: Mutex<Option<ValidityFlag>>,
    last_run: Tick,
    this_run: Tick,
    func_name: String,
    command_error_sink: CommandErrorSink,
    parity_trace: Option<ParityRunHandle>,
}

// SAFETY: the cache pointers and the lifetime-erased cell are dereferenced
// only after `validity` admits access, which `check_thread` limits to the
// thread running the system; see `PyParamSet::new` for why they are live then.
unsafe impl Send for PyParamSet {}
// SAFETY: as above; the active member window sits behind a mutex.
unsafe impl Sync for PyParamSet {}

impl PyParamSet {
    /// # Safety
    /// Every cache pointer in `members` must point into the system's
    /// scheduled run state, which only `initialize` rebuilds and which
    /// therefore stays in place for the whole run, and `world` must be the
    /// run's cell; the run invalidates `validity` before either ends.
    #[allow(clippy::too_many_arguments)]
    pub(crate) unsafe fn new(
        descriptors: &Arc<Vec<SystemParam>>,
        members: Vec<ParamSetMember>,
        world: UnsafeWorldCell<'_>,
        validity: ValidityFlag,
        last_run: Tick,
        this_run: Tick,
        func_name: String,
        command_error_sink: CommandErrorSink,
        parity_trace: Option<ParityRunHandle>,
    ) -> Self {
        // SAFETY: the validity flag fences every use of the erased cell.
        let world =
            unsafe { std::mem::transmute::<UnsafeWorldCell<'_>, UnsafeWorldCell<'static>>(world) };
        debug_assert_eq!(descriptors.len(), members.len());
        Self {
            descriptors: Arc::downgrade(descriptors),
            members,
            world,
            validity,
            active: Mutex::new(None),
            last_run,
            this_run,
            func_name,
            command_error_sink,
            parity_trace,
        }
    }

    fn member(&self, py: Python<'_>, index: usize) -> PyResult<Py<PyAny>> {
        self.validity
            .check_read()
            .map_err(|_| PyRuntimeError::new_err(PARAM_SET_OUTSIDE_SYSTEM))?;
        let member = self.members.get(index).ok_or_else(|| {
            PyRuntimeError::new_err(param_set_member_unavailable(self.members.len(), index))
        })?;
        // The run's parameter list holds the descriptors until it ends.
        let descriptors = self
            .descriptors
            .upgrade()
            .ok_or_else(|| PyRuntimeError::new_err(PARAM_SET_OUTSIDE_SYSTEM))?;
        let Some(param) = descriptors.get(index) else {
            return Err(PyRuntimeError::new_err(param_set_member_unavailable(
                descriptors.len(),
                index,
            )));
        };
        // End the previous member's borrow before opening this one.
        let window = self.validity.new_child();
        if let Some(previous) = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .replace(window.clone())
        {
            previous.set_invalid();
        }
        let query_caches = member
            .query_cache
            // SAFETY: `validity`, checked above, is still open, so the
            // scheduled run state this points into is in place (see `new`).
            .map(|cache| std::slice::from_ref(unsafe { cache.as_ref() }))
            .unwrap_or_default();
        let view_caches = member
            .view_cache
            // SAFETY: as above.
            .map(|cache| std::slice::from_ref(unsafe { cache.as_ref() }))
            .unwrap_or_default();
        let cursors = member.message_cursor.as_slice();
        let mut commands: Option<Commands<'_, '_>> = None;
        let mut args: SmallVec<[Py<PyAny>; 8]> = SmallVec::new();
        // SAFETY: the caches were built from this member's descriptor for the
        // system's declared access, and `window` closes with `validity`.
        let error = unsafe {
            build_run_args(
                py,
                std::slice::from_ref(param),
                query_caches,
                view_caches,
                &[],
                cursors,
                &mut commands,
                &mut args,
                self.world,
                &window,
                self.last_run,
                self.this_run,
                &self.func_name,
                &self.command_error_sink,
                self.parity_trace.clone(),
            )
        };
        if let Some(error) = error {
            window.set_invalid();
            return Err(error);
        }
        args.pop()
            .ok_or_else(|| PyRuntimeError::new_err("ParamSet member produced no value"))
    }
}

#[pymethods]
impl PyParamSet {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    fn __class_getitem__(
        _cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<PyParamSetParam> {
        let origin = key
            .getattr("__origin__")
            .map_err(|_| PyTypeError::new_err(PARAM_SET_EXPECTS_TUPLE))?;
        if !origin.is(PyTuple::type_object(key.py())) {
            return Err(PyTypeError::new_err(PARAM_SET_EXPECTS_TUPLE));
        }
        let args = key.getattr("__args__")?;
        let args = args.cast::<PyTuple>()?;
        if args.is_empty() || args.len() > MAX_PARAM_SET_MEMBERS {
            return Err(PyTypeError::new_err(PARAM_SET_MEMBER_COUNT));
        }
        Ok(PyParamSetParam {
            members: args.iter().map(Bound::unbind).collect(),
        })
    }

    fn p0(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.member(py, 0)
    }
    fn p1(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.member(py, 1)
    }
    fn p2(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.member(py, 2)
    }
    fn p3(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.member(py, 3)
    }
    fn p4(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.member(py, 4)
    }
    fn p5(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.member(py, 5)
    }
    fn p6(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.member(py, 6)
    }
    fn p7(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.member(py, 7)
    }
}
