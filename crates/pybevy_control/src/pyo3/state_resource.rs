//! Control-plane access to `State[T]` and `NextState[T]`.
//!
//! Both are PyO3-backed and hold their value behind the accessors Bevy
//! declares (`get`, `set`, `reset`), with no properties for the field scan to
//! find. Rather than add a Python property Bevy has no counterpart for, the
//! control plane reads and writes them here under the same synthetic `variant`
//! key it already synthesizes for enum components.

use pyo3::{prelude::*, types::PyType};

pub(crate) const VARIANT: &str = "variant";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateResource {
    Current,
    Next,
}

/// Identify a state resource by Python type name.
///
/// Matching on the name rather than the type keeps this crate off a dependency
/// on the root `pybevy` crate, which owns `PyState`. The same idiom serves
/// `Color` and `Entity` in the value serializer.
pub(crate) fn classify(bound: &Bound<'_, PyAny>) -> Option<StateResource> {
    let py_type = bound.get_type();
    let name = py_type.name().ok()?;
    match name.to_str().ok()? {
        "State" => Some(StateResource::Current),
        "NextState" => Some(StateResource::Next),
        _ => None,
    }
}

/// The active or pending member name, or `None` when no transition is queued.
pub(crate) fn read_variant(
    bound: &Bound<'_, PyAny>,
    kind: StateResource,
) -> PyResult<Option<String>> {
    let member = match kind {
        StateResource::Current => bound.call_method0("get")?,
        StateResource::Next => bound.call_method0("peek_pending")?,
    };
    if member.is_none() {
        return Ok(None);
    }
    Ok(Some(member.getattr("name")?.extract::<String>()?))
}

/// Queue a transition by member name; `None` cancels a pending one.
///
/// Only `NextState` is writable, matching Bevy: `State` is changed by queueing
/// a transition, never assigned.
pub(crate) fn write_variant(
    bound: &Bound<'_, PyAny>,
    kind: StateResource,
    variant: Option<&str>,
) -> Result<(), String> {
    if kind != StateResource::Next {
        return Err("State is read-only; queue a transition through NextState instead".to_string());
    }
    let Some(variant) = variant else {
        return bound
            .call_method0("reset")
            .map(|_| ())
            .map_err(|error| error.to_string());
    };
    let member = state_member(bound, variant)?;
    bound
        .call_method1("set", (member,))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Resolve a member name against the machine's own enum, so an unknown name is
/// rejected here rather than queueing nothing.
fn state_member<'py>(
    bound: &Bound<'py, PyAny>,
    variant: &str,
) -> Result<Bound<'py, PyAny>, String> {
    let state_type = bound
        .call_method0("state_type")
        .map_err(|error| error.to_string())?;
    state_type.getattr(variant).map_err(|_| {
        let label = state_type
            .cast::<PyType>()
            .ok()
            .and_then(|cls| cls.name().ok().map(|name| name.to_string()))
            .unwrap_or_else(|| "the state type".to_string());
        format!("'{variant}' is not a member of {label}")
    })
}
