use pyo3::{
    prelude::*,
    types::{PyModule, PyType},
};

use crate::handlers::reflect_mutate::compiled_type_is_enum;

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
    let pybevy_wrapper = owner.module().is_ok_and(|module| {
        let module = module.to_string_lossy();
        module == "pybevy" || module.starts_with("pybevy.")
    });
    pybevy_wrapper
        && owner
            .name()
            .is_ok_and(|name| compiled_type_is_enum(&name.to_string_lossy()))
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
