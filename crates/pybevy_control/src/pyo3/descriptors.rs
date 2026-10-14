use pyo3::prelude::*;

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
