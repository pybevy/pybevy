//! PyO3 payload conversion for the interpreter-neutral parity trace.

use std::collections::BTreeMap;

use pybevy_core::PyHandle;
use pybevy_ecs::shared::parity_trace::CanonValue;
use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple},
};

const MAX_CANON_DEPTH: usize = 64;

pub(crate) fn canonicalize_payload(value: &Bound<'_, PyAny>) -> PyResult<CanonValue> {
    canonicalize_inner(value, 0, None)
}

pub(crate) fn canonicalize_payload_without_root_field(
    value: &Bound<'_, PyAny>,
    field_name: &str,
) -> PyResult<CanonValue> {
    canonicalize_inner(value, 0, Some(field_name))
}

fn canonicalize_inner(
    value: &Bound<'_, PyAny>,
    depth: usize,
    omitted_root_field: Option<&str>,
) -> PyResult<CanonValue> {
    if depth >= MAX_CANON_DEPTH {
        return Err(unhashable(value, "payload nesting exceeds 64 levels"));
    }
    if value.is_none() {
        return Ok(CanonValue::None);
    }
    if value.is_instance_of::<PyBool>() {
        return Ok(CanonValue::Bool(value.extract()?));
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(CanonValue::Int(value.str()?.to_string()));
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(CanonValue::Float);
    }
    if value.is_instance_of::<PyString>() {
        return Ok(CanonValue::String(value.extract()?));
    }
    if let Ok(handle) = value.extract::<PyRef<'_, PyHandle>>() {
        return Ok(CanonValue::Map(BTreeMap::from([
            (
                "asset_type".to_string(),
                CanonValue::String(handle.asset_type_name().unwrap_or("Unknown").to_string()),
            ),
            (
                "asset_id".to_string(),
                CanonValue::Int(handle.id().to_string()),
            ),
        ])));
    }
    if let Ok(dictionary) = value.cast::<PyDict>() {
        let mut result = BTreeMap::new();
        for (key, item) in dictionary.iter() {
            let key = key
                .extract::<String>()
                .map_err(|_| unhashable(value, "mapping payloads must contain only string keys"))?;
            result.insert(
                key,
                canonicalize_inner(&item, depth + 1, omitted_root_field)?,
            );
        }
        return Ok(CanonValue::Map(result));
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let items = value
            .try_iter()?
            .map(|item| canonicalize_inner(&item?, depth + 1, omitted_root_field))
            .collect::<PyResult<Vec<_>>>()?;
        return Ok(CanonValue::List(items));
    }

    let value_type = value.get_type();
    let mut fields = BTreeMap::new();
    if let Ok(annotations) = value_type.getattr("__annotations__")
        && let Ok(annotations) = annotations.cast::<PyDict>()
    {
        for (name, _) in annotations.iter() {
            let name = name.extract::<String>().map_err(|_| {
                unhashable(value, "annotated payload fields must have string names")
            })?;
            if name.starts_with('_') || (depth == 0 && omitted_root_field == Some(name.as_str())) {
                continue;
            }
            let field = value.getattr(name.as_str()).map_err(|error| {
                unhashable(value, &format!("cannot read field {name}: {error}"))
            })?;
            fields.insert(
                name,
                canonicalize_inner(&field, depth + 1, omitted_root_field)?,
            );
        }
    }

    if let Ok(names) = value_type.dir() {
        for name in names.iter() {
            let Ok(name) = name.extract::<String>() else {
                continue;
            };
            if name.starts_with('_')
                || fields.contains_key(&name)
                || (depth == 0 && omitted_root_field == Some(name.as_str()))
            {
                continue;
            }
            let Ok(descriptor) = value_type.as_any().getattr(name.as_str()) else {
                continue;
            };
            let descriptor_kind = descriptor
                .get_type()
                .name()
                .map(|name| name.to_string())
                .unwrap_or_default();
            if descriptor_kind != "getset_descriptor" && descriptor_kind != "property" {
                continue;
            }
            let field = value.getattr(name.as_str()).map_err(|error| {
                unhashable(value, &format!("cannot read field {name}: {error}"))
            })?;
            fields.insert(
                name,
                canonicalize_inner(&field, depth + 1, omitted_root_field)?,
            );
        }
    }

    if fields.is_empty()
        && let Ok(instance_fields) = value.getattr("__dict__")
        && let Ok(instance_fields) = instance_fields.cast::<PyDict>()
    {
        for (name, field) in instance_fields.iter() {
            let name = name
                .extract::<String>()
                .map_err(|_| unhashable(value, "object payload fields must have string names"))?;
            if !name.starts_with('_') && !(depth == 0 && omitted_root_field == Some(name.as_str()))
            {
                fields.insert(
                    name,
                    canonicalize_inner(&field, depth + 1, omitted_root_field)?,
                );
            }
        }
    }
    let omitted_field_was_present =
        depth == 0 && omitted_root_field.is_some_and(|name| value.hasattr(name).unwrap_or(false));
    if !fields.is_empty() || omitted_field_was_present {
        return Ok(CanonValue::Map(fields));
    }

    let type_name = value_type.name()?.to_string();
    let representation = value.repr()?.to_string();
    let prefix = format!("{type_name}.");
    if let Some(variant) = representation.strip_prefix(&prefix) {
        let variant = variant.split_once('(').map_or(variant, |(name, _)| name);
        return Ok(CanonValue::Map(BTreeMap::from([(
            "variant".to_string(),
            CanonValue::String(variant.to_string()),
        )])));
    }

    // Fieldless native wrappers are marker values. The separately recorded
    // type name preserves their semantic identity.
    if value_type.is_subclass_of::<pybevy_core::PyComponent>()?
        || value_type.is_subclass_of::<pybevy_core::PyResource>()?
        || value_type.is_subclass_of::<pybevy_core::PyMessage>()?
    {
        return Ok(CanonValue::Map(BTreeMap::new()));
    }

    Err(unhashable(value, "unsupported payload shape"))
}

fn unhashable(value: &Bound<'_, PyAny>, reason: &str) -> PyErr {
    let type_name = value
        .get_type()
        .name()
        .map(|name| name.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    PyTypeError::new_err(format!(
        "parity trace payload of type `{type_name}` is unhashable: {reason}"
    ))
}
