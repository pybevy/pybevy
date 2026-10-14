//! PyO3 adapter for the `@system`, `@condition` and `@observer` decorators.
//!
//! The decorator classes are shared Python (`pybevy/ecs/_callables.py`); this
//! module only recognizes them at scheduler entry points and hands back the
//! wrapped function, so native holders keep storing raw callables.

use pybevy_core::public_error;
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    sync::PyOnceLock,
    types::PyType,
};

/// The scheduler role a decorated callable was marked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CallableKind {
    System,
    Condition,
    Observer,
}

static SYSTEM_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();
static CONDITION_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();
static OBSERVER_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();

impl CallableKind {
    const ALL: [Self; 3] = [Self::System, Self::Condition, Self::Observer];

    fn class_name(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Condition => "SystemCondition",
            Self::Observer => "ObserverSystem",
        }
    }

    pub(crate) fn decorator(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Condition => "condition",
            Self::Observer => "observer",
        }
    }

    fn from_decorator(name: &str) -> PyResult<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.decorator() == name)
            .ok_or_else(|| PyValueError::new_err(format!("unknown callable kind {name:?}")))
    }

    fn class<'py>(self, py: Python<'py>) -> PyResult<&'py Bound<'py, PyType>> {
        let cell = match self {
            Self::System => &SYSTEM_TYPE,
            Self::Condition => &CONDITION_TYPE,
            Self::Observer => &OBSERVER_TYPE,
        };
        cell.get_or_try_init(py, || {
            Ok(py
                .import("pybevy.ecs")?
                .getattr(self.class_name())?
                .cast_into::<PyType>()?
                .unbind())
        })
        .map(|class| class.bind(py))
    }
}

/// The role `value` was decorated for, or `None` for anything undecorated.
pub(crate) fn decorated_kind(value: &Bound<'_, PyAny>) -> PyResult<Option<CallableKind>> {
    // Plain functions skip the class lookup; every decorated callable has `__wrapped__`.
    if !value.hasattr("__wrapped__")? {
        return Ok(None);
    }
    for kind in CallableKind::ALL {
        if value.is_instance(kind.class(value.py())?)? {
            return Ok(Some(kind));
        }
    }
    Ok(None)
}

fn display_name(value: &Bound<'_, PyAny>) -> String {
    ["__qualname__", "__name__"]
        .into_iter()
        .find_map(|attr| value.getattr(attr).ok()?.extract::<String>().ok())
        .unwrap_or_else(|| {
            value
                .repr()
                .map(|repr| repr.to_string())
                .unwrap_or_else(|_| "<callable>".to_owned())
        })
}

/// The function wrapped by a `kind` decorator, or the `TypeError` naming the fix.
pub(crate) fn unwrap_decorated<'py>(
    value: &Bound<'py, PyAny>,
    kind: CallableKind,
    entry: &str,
) -> PyResult<Bound<'py, PyAny>> {
    match decorated_kind(value)? {
        Some(found) if found == kind => value.getattr("__wrapped__"),
        Some(found) => Err(PyTypeError::new_err(
            public_error::wrong_callable_decorator(
                entry,
                &display_name(value),
                found.decorator(),
                kind.decorator(),
            ),
        )),
        None if value.is_callable() => {
            let short_name = value
                .getattr("__name__")
                .and_then(|name| name.extract::<String>())
                .unwrap_or_else(|_| "fn".to_owned());
            Err(PyTypeError::new_err(public_error::undecorated_callable(
                entry,
                &display_name(value),
                &short_name,
                kind.decorator(),
            )))
        }
        None => Err(PyTypeError::new_err(
            public_error::non_callable_for_decorator(
                entry,
                &value.get_type().name()?.to_string(),
                kind.decorator(),
            ),
        )),
    }
}

/// `value` itself, or the function it wraps when decorated as `kind`.
pub(crate) fn strip_decorated<'py>(
    value: &Bound<'py, PyAny>,
    kind: CallableKind,
) -> PyResult<Bound<'py, PyAny>> {
    if value.hasattr("__wrapped__")? && value.is_instance(kind.class(value.py())?)? {
        value.getattr("__wrapped__")
    } else {
        Ok(value.clone())
    }
}

/// Shared-wording check used by the Python condition combinators.
#[pyfunction]
#[pyo3(name = "_unwrap_decorated", signature = (value, kind, entry, /))]
pub(crate) fn py_unwrap_decorated<'py>(
    value: &Bound<'py, PyAny>,
    kind: &str,
    entry: &str,
) -> PyResult<Bound<'py, PyAny>> {
    unwrap_decorated(value, CallableKind::from_decorator(kind)?, entry)
}
