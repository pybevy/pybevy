use pyo3::{prelude::*, types::PyType};

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
