//! Turns "'type' object is not an instance of 'X'" into an actionable error.
//!
//! A data-carrying pyenum exposes every variant as a class, so `AlphaMode.Add`
//! without parentheses is a type, not a value. Unit-only enums such as `Msaa`
//! expose plain values, so the caller cannot tell the two apart at the call
//! site. Everything here is error-path only.

use pybevy_core::public_error::{
    has_variant_hint, parse_not_an_instance, variant_constructor_hint,
};
use pyo3::{prelude::*, types::PyType};

use crate::ecs::kwarg_hint::resolve_callable;

/// Nested variant class names of `class`, in declaration order.
///
/// A variant is an entry in the class `__dict__` that is itself a type and a
/// strict subclass of the enum, which is exactly how `pyenum` emits
/// data-carrying variants. `__dict__` is used rather than `dir()` because it
/// preserves declaration order and excludes inherited attributes.
fn variant_names(class: &Bound<'_, PyAny>) -> Option<Vec<String>> {
    let ty = class.cast::<PyType>().ok()?;
    let dict = ty.getattr("__dict__").ok()?;
    let mut names = Vec::new();
    for key in dict.try_iter().ok()? {
        let Ok(name) = key.ok()?.extract::<String>() else {
            continue;
        };
        if name.starts_with('_') {
            continue;
        }
        let Ok(attr) = ty.getattr(name.as_str()) else {
            continue;
        };
        let Ok(attr_ty) = attr.cast_into::<PyType>() else {
            continue;
        };
        if attr_ty.is_subclass(ty).unwrap_or(false) && !attr_ty.is(ty) {
            names.push(name);
        }
    }
    (!names.is_empty()).then_some(names)
}

/// The hint to append to `message`, or None to leave the error untouched.
pub(crate) fn hint_for(
    py: Python<'_>,
    traceback: Option<&Bound<'_, PyAny>>,
    message: &str,
) -> Option<String> {
    if has_variant_hint(message) {
        return None;
    }
    let parsed = parse_not_an_instance(message)?;
    // Only a passed class can be a missing-parentheses mistake.
    if parsed.actual != "type" {
        return None;
    }
    let class = resolve_callable(py, traceback, parsed.expected)?;
    let names = variant_names(&class)?;
    let borrowed: Vec<&str> = names.iter().map(String::as_str).collect();
    variant_constructor_hint(parsed.expected, &borrowed)
}
