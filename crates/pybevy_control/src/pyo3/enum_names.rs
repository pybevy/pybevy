use pyo3::{
    prelude::*,
    types::{PyModule, PyType},
};

use crate::codec::reflect::{compiled_enum_variant_info, compiled_type_is_enum};

pub(super) fn is_enum_type(owner: &Bound<'_, PyType>) -> bool {
    owner.hasattr("__pybevy_variant_aliases__").unwrap_or(false)
        || is_python_enum(owner)
        || is_reflected_bevy_enum(owner)
}

fn is_python_enum(owner: &Bound<'_, PyType>) -> bool {
    PyModule::import(owner.py(), "enum")
        .and_then(|module| module.getattr("Enum"))
        .and_then(|base| owner.is_subclass(&base))
        .unwrap_or(false)
}

fn is_reflected_bevy_enum(owner: &Bound<'_, PyType>) -> bool {
    reflected_bevy_enum_name(owner).is_some()
}

fn reflected_bevy_enum_name(owner: &Bound<'_, PyType>) -> Option<String> {
    let module = owner.module().ok()?;
    let module = module.to_string_lossy();
    if module != "pybevy" && !module.starts_with("pybevy.") {
        return None;
    }
    let name = owner.name().ok()?.to_string_lossy().into_owned();
    compiled_type_is_enum(&name).then_some(name)
}

/// Splits same-type class attributes into unit variants and constants.
pub(super) fn partition_unit_variants(
    owner: &Bound<'_, PyType>,
    instances: Vec<String>,
    has_variant_classes: bool,
) -> (Vec<String>, Vec<String>) {
    if is_python_enum(owner) {
        return (instances, Vec::new());
    }
    let Some(owner_name) = reflected_bevy_enum_name(owner) else {
        return if has_variant_classes {
            (Vec::new(), instances)
        } else {
            (instances, Vec::new())
        };
    };
    let aliases = variant_aliases(owner);
    instances.into_iter().partition(|name| {
        let native = aliases
            .iter()
            .find_map(|(native, python)| (python == name).then_some(native.as_str()))
            .unwrap_or(name);
        compiled_enum_variant_info(&owner_name, native).is_some()
    })
}

fn variant_aliases(owner: &Bound<'_, PyType>) -> Vec<(String, String)> {
    owner
        .getattr("__pybevy_variant_aliases__")
        .ok()
        .and_then(|value| value.extract().ok())
        .unwrap_or_default()
}

pub(super) fn python_variant_name(owner: &Bound<'_, PyType>, name: &str) -> String {
    variant_aliases(owner)
        .into_iter()
        .find_map(|(native, python)| (native == name).then_some(python))
        .unwrap_or_else(|| name.to_string())
}

pub(super) fn canonical_variant_name(owner: &Bound<'_, PyType>, name: &str) -> String {
    variant_aliases(owner)
        .into_iter()
        .find_map(|(native, python)| (python == name).then_some(native))
        .unwrap_or_else(|| name.to_string())
}

pub(super) fn accepted_variant_names(
    owner: &Bound<'_, PyType>,
    mut names: Vec<String>,
) -> Vec<String> {
    for (native, python) in variant_aliases(owner) {
        if names.iter().any(|name| name == &python) {
            names.push(native);
        }
    }
    names.sort();
    names.dedup();
    names
}
