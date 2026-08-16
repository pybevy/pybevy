//! Turns a bare "unexpected keyword argument" TypeError into an actionable one.
//!
//! PyO3 raises that TypeError inside its own generated argument extraction,
//! before `#[new]` runs, so there is no constructor hook. Instead this runs at
//! the system-error boundary, where the exception is already caught and the GIL
//! is already held. Everything here is error-path only.

use pybevy_core::public_error::{
    has_kwarg_hint, parse_unexpected_keyword, unexpected_keyword_hint,
};
use pyo3::{
    prelude::*,
    types::{PyDict, PyType},
};

/// Parameter names of `callable`, in declaration order.
fn parameter_names(py: Python<'_>, callable: &Bound<'_, PyAny>) -> Option<Vec<String>> {
    let signature = py
        .import("inspect")
        .ok()?
        .call_method1("signature", (callable,))
        .ok()?;
    let parameters = signature.getattr("parameters").ok()?;
    let mut names = Vec::new();
    for key in parameters.call_method0("keys").ok()?.try_iter().ok()? {
        let name: String = key.ok()?.extract().ok()?;
        // *args / **kwargs are not suggestible keywords.
        if !name.starts_with('*') {
            names.push(name);
        }
    }
    (!names.is_empty()).then_some(names)
}

/// Look `name` up where the failing code would have seen it.
///
/// Checked against `__name__` so a shadowing local cannot produce a confident
/// hint about the wrong class. Nothing is cached: hot reload rebuilds Python
/// types, and stale type pointers are exactly what CLAUDE.md forbids holding.
fn resolve_callable<'py>(py: Python<'py>, error: &PyErr, name: &str) -> Option<Bound<'py, PyAny>> {
    let matches = |value: &Bound<'py, PyAny>| -> bool {
        value.is_instance_of::<PyType>()
            && value
                .getattr("__name__")
                .ok()
                .and_then(|n| n.extract::<String>().ok())
                .is_some_and(|n| n == name)
    };

    // The frame that raised is the one that knows which `RectLight` it meant.
    let mut frame = error
        .traceback(py)
        .and_then(|tb| tb.getattr("tb_frame").ok());
    while let Some(current) = frame {
        for namespace in ["f_locals", "f_globals"] {
            if let Ok(scope) = current.getattr(namespace)
                && let Ok(dict) = scope.cast_into::<PyDict>()
                && let Ok(Some(found)) = dict.get_item(name)
                && matches(&found)
            {
                return Some(found);
            }
        }
        frame = current.getattr("f_back").ok().filter(|f| !f.is_none());
    }

    // Aliased imports (`from pybevy.light import RectLight as RL`) leave no
    // binding under the real name, so fall back to pybevy's own modules.
    let modules = py.import("sys").ok()?.getattr("modules").ok()?;
    let modules = modules.cast_into::<PyDict>().ok()?;
    for (key, module) in modules.iter() {
        let Ok(module_name) = key.extract::<String>() else {
            continue;
        };
        if !module_name.starts_with("pybevy") {
            continue;
        }
        if let Ok(found) = module.getattr(name)
            && matches(&found)
        {
            return Some(found);
        }
    }
    None
}

/// The hint to append to `message`, or None to leave the error untouched.
pub(crate) fn hint_for(py: Python<'_>, error: &PyErr, message: &str) -> Option<String> {
    // CPython already suggests for pure-Python callables such as @component
    // dataclasses, and the binding layer may one day suggest at raise time.
    // Never hint twice.
    if has_kwarg_hint(message) {
        return None;
    }
    let parsed = parse_unexpected_keyword(message)?;
    let callable = resolve_callable(py, error, parsed.callable)?;
    let names = parameter_names(py, &callable)?;
    let borrowed: Vec<&str> = names.iter().map(String::as_str).collect();
    unexpected_keyword_hint(parsed.callable, parsed.keyword, &borrowed)
}

/// Rewrite `error` in place to carry a suggestion, when one applies.
///
/// Appends to the exception's own message rather than using `add_note`, so the
/// result reads exactly like CPython's built-in suggestion and every consumer
/// of the message (stderr, the re-raise from `app.run()`, `LastSystemError`,
/// MCP `get_last_error`) picks it up without special-casing notes.
pub(crate) fn enrich(py: Python<'_>, error: &PyErr) {
    let message = error.to_string();
    // `PyErr::to_string` renders "TypeError: msg"; the parser tolerates that.
    let Some(hint) = hint_for(py, error, &message) else {
        return;
    };
    let value = error.value(py);
    let current = value.str().ok().and_then(|s| s.extract::<String>().ok());
    let Some(current) = current else { return };
    let _ = value.setattr("args", (format!("{current}. {hint}"),));
}
