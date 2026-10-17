use pyo3::{
    exceptions::PyAttributeError,
    prelude::*,
    types::{PyCFunction, PyType},
};

/// Whether a class attribute is a PyO3 getset descriptor.
pub(crate) fn is_getset_descriptor(attribute: &Bound<'_, PyAny>) -> bool {
    attribute
        .get_type()
        .name()
        .is_ok_and(|kind| kind == "getset_descriptor")
}

/// Whether a class attribute is a PyO3 getset descriptor or a Python property.
pub(crate) fn is_field_descriptor(attribute: &Bound<'_, PyAny>) -> bool {
    attribute
        .get_type()
        .name()
        .is_ok_and(|kind| kind == "getset_descriptor" || kind == "property")
}

/// Whether `probe` accepts its own value for `name`. PyO3 getset descriptors
/// do not expose a property's `fset`, so this tells a getter from a
/// getter/setter; probe an owned copy, never a live value.
pub(crate) fn accepts_own_value(probe: &Bound<'_, PyAny>, name: &str) -> bool {
    probe
        .getattr(name)
        .and_then(|value| probe.setattr(name, value))
        .is_ok()
}

/// Whether setting `name` raises `AttributeError`, probed on a detached copy.
pub(crate) fn rejects_attribute_write(instance: &Bound<'_, PyAny>, name: &str) -> bool {
    let Ok(probe) = instance
        .call_method0("__copy__")
        .or_else(|_| instance.get_type().call0())
    else {
        return false;
    };
    probe
        .getattr(name)
        .and_then(|value| probe.setattr(name, value))
        .is_err_and(|error| error.is_instance_of::<PyAttributeError>(instance.py()))
}

/// The parameter names of a class's constructor signature.
pub(crate) fn constructor_parameter_names(class: &Bound<'_, PyType>) -> Option<Vec<String>> {
    class
        .py()
        .import("inspect")
        .ok()?
        .call_method1("signature", (class,))
        .ok()?
        .getattr("parameters")
        .ok()?
        .try_iter()
        .ok()?
        .map(|name| name.ok()?.extract::<String>().ok())
        .collect()
}

/// A value's constructor parameters paired with their current values, read
/// through a same-named or `get_`-prefixed zero-argument method. `UiRect.left`
/// is Bevy's constructor, so its field reads through `get_left()`.
pub(crate) fn constructor_fields<'py>(
    value: &Bound<'py, PyAny>,
) -> Option<Vec<(String, Bound<'py, PyAny>)>> {
    let names = constructor_parameter_names(&value.get_type())?;
    if names.is_empty() {
        return None;
    }
    names
        .into_iter()
        .map(|name| {
            let current = [name.clone(), format!("get_{name}")]
                .iter()
                .find_map(|accessor| {
                    let accessor = value.getattr(accessor.as_str()).ok()?;
                    accessor.cast::<PyCFunction>().ok()?;
                    accessor.call0().ok()
                })?;
            Some((name, current))
        })
        .collect()
}
