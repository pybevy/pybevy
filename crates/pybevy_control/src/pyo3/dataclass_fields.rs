use std::collections::BTreeSet;

use pyo3::{
    prelude::*,
    types::{PyModule, PyType},
};

pub(super) fn dataclass_field_names(
    class: &Bound<'_, PyType>,
) -> PyResult<Option<BTreeSet<String>>> {
    let dataclasses = PyModule::import(class.py(), "dataclasses")?;
    let is_dataclass = dataclasses
        .call_method1("is_dataclass", (class,))?
        .extract::<bool>()?;
    if !is_dataclass {
        return Ok(None);
    }

    let fields = dataclasses.call_method1("fields", (class,))?;
    let mut names = BTreeSet::new();
    for field in fields.try_iter()? {
        names.insert(field?.getattr("name")?.extract::<String>()?);
    }
    Ok(Some(names))
}
