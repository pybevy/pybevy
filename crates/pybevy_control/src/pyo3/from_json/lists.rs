use pyo3::{
    prelude::*,
    types::{
        PyBool, PyCFunction, PyDict, PyFloat, PyInt, PyList, PyModule, PySet, PyString, PyTuple,
        PyType,
    },
};

use crate::{
    codec::value::json_kind_name,
    pyo3::{
        enum_names,
        from_json::{
            enums::{
                VariantSource, construct_enum_variant, enum_owner_type, enum_variant_names,
                same_type_class_attributes,
            },
            field::{convert_field_value, json_to_py},
            math::math_value_from_json,
        },
    },
};

enum ListElementTarget<'py> {
    Generic,
    Wrapper {
        class: Bound<'py, PyType>,
        instance: Option<Bound<'py, PyAny>>,
    },
}

pub(crate) fn convert_list_elements<'py>(
    py: Python<'py>,
    component: &Bound<'py, PyAny>,
    field_name: &str,
    current: &Bound<'py, PyAny>,
    arr: &[serde_json::Value],
) -> Result<Option<Py<PyAny>>, String> {
    let ListElementTarget::Wrapper { class, instance } =
        list_element_target(py, component, field_name, current)?
    else {
        return Ok(None);
    };
    let class_name = class
        .name()
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .into_owned();
    let math_shape = match class_name.as_str() {
        "Vec2" => Some("[x, y]"),
        "Vec3" => Some("[x, y, z]"),
        "Vec4" | "Quat" => Some("[x, y, z, w]"),
        _ => None,
    };
    let is_enum = instance
        .as_ref()
        .is_some_and(|instance| !enum_variant_names(&enum_owner_type(instance)).is_empty());
    let items = arr
        .iter()
        .enumerate()
        .map(|(index, value)| -> Result<Py<PyAny>, String> {
            if let Some(shape) = math_shape {
                if let serde_json::Value::Array(values) = value {
                    if let Some(result) = math_value_from_json(py, &class_name, values) {
                        return result;
                    }
                    return Err(format!(
                        "element {index}: {class_name} expects {shape}, got {} elements",
                        values.len()
                    ));
                }
                return Err(format!(
                    "element {index}: {class_name} expects {shape}, got {}",
                    json_kind_name(value)
                ));
            }
            match value {
                serde_json::Value::Object(fields) => {
                    if is_enum {
                        let instance = instance.as_ref().ok_or_else(|| {
                            format!("element {index}: expected a {class_name} element")
                        })?;
                        convert_enum_element(py, instance, index, fields)
                    } else {
                        convert_struct_element(py, &class, instance.as_ref(), index, fields)
                    }
                }
                _ => Err(format!(
                    "element {index}: expected object, got {}",
                    json_kind_name(value)
                )),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    PyList::new(py, items)
        .map(|list| list.unbind().into_any())
        .map(Some)
        .map_err(|error| error.to_string())
}

fn list_element_target<'py>(
    py: Python<'py>,
    component: &Bound<'py, PyAny>,
    field_name: &str,
    current: &Bound<'py, PyAny>,
) -> Result<ListElementTarget<'py>, String> {
    if let Ok(first) = current.get_item(0usize) {
        if first.is_instance_of::<PyString>()
            || first.is_instance_of::<PyInt>()
            || first.is_instance_of::<PyFloat>()
            || first.is_instance_of::<PyBool>()
            || first.is_none()
            || first.is_instance_of::<PyDict>()
            || first.is_instance_of::<PyList>()
            || first.is_instance_of::<PySet>()
            || first.is_instance_of::<PyTuple>()
        {
            return Ok(ListElementTarget::Generic);
        }
        return Ok(ListElementTarget::Wrapper {
            class: first.get_type(),
            instance: Some(first),
        });
    }
    // Probe a detached value because the component can be a live resource borrow.
    let Ok(probe_owner) = component
        .call_method0("__copy__")
        .or_else(|_| component.get_type().call0())
    else {
        return Ok(ListElementTarget::Generic);
    };
    let probe = PyList::new(py, [PyDict::new(py)]).map_err(|error| error.to_string())?;
    match probe_owner.setattr(field_name, &probe) {
        Ok(()) => Ok(ListElementTarget::Generic),
        Err(error) => {
            const MARKER: &str = "is not an instance of '";
            let message = error.to_string();
            let class_name = message.rfind(MARKER).and_then(|position| {
                let rest = &message[position + MARKER.len()..];
                rest.find('\'').map(|end| rest[..end].to_string())
            });
            let Some(class_name) = class_name else {
                return Ok(ListElementTarget::Generic);
            };
            let Some(class) = find_pybevy_class(py, &class_name) else {
                return Ok(ListElementTarget::Generic);
            };
            let instance = constructible_representative(&class);
            Ok(ListElementTarget::Wrapper { class, instance })
        }
    }
}

pub(crate) fn find_pybevy_class<'py>(py: Python<'py>, name: &str) -> Option<Bound<'py, PyType>> {
    let sys = PyModule::import(py, "sys").ok()?;
    let modules_bound = sys.getattr("modules").ok()?;
    let modules = modules_bound.cast::<PyDict>().ok()?;
    for entry in modules.iter() {
        let (key, _value) = entry;
        let Ok(key) = key.extract::<String>() else {
            continue;
        };
        if !key.starts_with("pybevy.") {
            continue;
        }
        let Some(module) = modules.get_item(key.as_str()).ok().flatten() else {
            continue;
        };
        let Ok(candidate) = module.getattr(name) else {
            continue;
        };
        let Ok(candidate) = candidate.cast::<PyType>() else {
            continue;
        };
        let defined_here = candidate
            .getattr("__module__")
            .ok()
            .and_then(|module| module.extract::<String>().ok())
            .is_some_and(|module| module == key);
        if defined_here {
            return Some(candidate.clone());
        }
    }
    None
}

pub(crate) fn constructible_representative<'py>(
    class: &Bound<'py, PyType>,
) -> Option<Bound<'py, PyAny>> {
    if let Ok(instance) = class.call0() {
        return Some(instance);
    }
    let (mut names, classes) = same_type_class_attributes(class);
    names.extend(classes);
    names.sort();
    for name in names {
        let Ok(candidate) = class.getattr(name.as_str()) else {
            continue;
        };
        if candidate.is_instance(class).unwrap_or(false) {
            return Some(candidate);
        }
        let instance = candidate
            .cast::<PyType>()
            .ok()
            .and_then(|variant| variant.call0().ok());
        if let Some(instance) = instance {
            return Some(instance);
        }
    }
    None
}

fn convert_struct_element(
    py: Python<'_>,
    class: &Bound<'_, PyType>,
    instance: Option<&Bound<'_, PyAny>>,
    index: usize,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<Py<PyAny>, String> {
    let class_name = class
        .name()
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .into_owned();
    let kwargs = PyDict::new(py);
    for (name, value) in fields {
        if !class.hasattr(name.as_str()).unwrap_or(false) {
            return Err(format!(
                "element {index}: {class_name} has no field '{name}'"
            ));
        }
        let converted = match instance {
            Some(instance) => {
                let owner = method_field_owner(py, instance, name);
                convert_field_value(py, owner.as_ref().unwrap_or(instance), name, value)
                    .map_err(|error| format!("{name}: {error}"))?
            }
            None => json_to_py(py, value)?,
        };
        kwargs
            .set_item(name, converted)
            .map_err(|error| error.to_string())?;
    }
    if let Some(required) = missing_constructor_field(class, fields.keys()) {
        return Err(format!(
            "element {index}: {class_name} requires field '{required}'"
        ));
    }
    class
        .call((), Some(&kwargs))
        .map(Bound::unbind)
        .map_err(|error| format!("element {index}: invalid {class_name} element: {error}"))
}

fn method_field_owner<'py>(
    py: Python<'py>,
    instance: &Bound<'py, PyAny>,
    name: &str,
) -> Option<Bound<'py, PyAny>> {
    let accessor = instance.getattr(name).ok()?;
    accessor.cast::<PyCFunction>().ok()?;
    let value = accessor.call0().ok()?;
    let fields = PyDict::new(py);
    fields.set_item(name, value).ok()?;
    PyModule::import(py, "types")
        .ok()?
        .getattr("SimpleNamespace")
        .ok()?
        .call((), Some(&fields))
        .ok()
}

fn enum_element_missing_field(
    owner: &Bound<'_, PyType>,
    variant_name: &str,
    payload: &serde_json::Value,
) -> Option<String> {
    let serde_json::Value::Object(fields) = payload else {
        return None;
    };
    if fields.is_empty() {
        return None;
    }
    let python_name = enum_names::python_variant_name(owner, variant_name);
    if !enum_variant_names(owner)
        .iter()
        .any(|name| name == &python_name)
    {
        return None;
    }
    let variant = owner.getattr(python_name.as_str()).ok()?;
    let variant_type = variant.cast::<PyType>().ok()?;
    missing_constructor_field(variant_type, fields.keys())
}

fn convert_enum_element(
    py: Python<'_>,
    instance: &Bound<'_, PyAny>,
    index: usize,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<Py<PyAny>, String> {
    let owner = enum_owner_type(instance);
    let owner_name = owner
        .name()
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .into_owned();
    if let Some(serde_json::Value::String(variant_name)) = fields.get("variant") {
        let mut payload = fields.clone();
        payload.remove("variant");
        let payload = if payload.is_empty() {
            serde_json::Value::Null
        } else if payload.len() == 1 && payload.contains_key("value") {
            payload.remove("value").unwrap()
        } else {
            serde_json::Value::Object(payload)
        };
        if let Some(field) = enum_element_missing_field(&owner, variant_name, &payload) {
            return Err(format!(
                "element {index}: variant '{variant_name}' requires field '{field}'"
            ));
        }
        return construct_enum_variant(py, instance, variant_name, &payload, VariantSource::Named)
            .map_err(|error| format!("element {index}: {error}"))?
            .ok_or_else(|| format!("element {index}: expected a {owner_name} element"));
    }
    if fields.len() == 1 {
        let (variant_name, variant_value) = fields.iter().next().unwrap();
        if let Some(field) = enum_element_missing_field(&owner, variant_name, variant_value) {
            return Err(format!(
                "element {index}: variant '{variant_name}' requires field '{field}'"
            ));
        }
        return construct_enum_variant(
            py,
            instance,
            variant_name,
            variant_value,
            VariantSource::ObjectKey,
        )
        .map_err(|error| format!("element {index}: {error}"))?
        .ok_or_else(|| format!("element {index}: expected a {owner_name} element"));
    }
    Err(format!(
        "element {index}: expected a {owner_name} element with a single variant key"
    ))
}

fn missing_constructor_field<'a>(
    class: &Bound<'_, PyType>,
    given: impl Iterator<Item = &'a String>,
) -> Option<String> {
    let py = class.py();
    let inspect = PyModule::import(py, "inspect").ok()?;
    let signature = inspect.getattr("signature").ok()?.call1((class,)).ok()?;
    let parameters = signature.getattr("parameters").ok()?;
    let empty = inspect.getattr("Parameter").ok()?.getattr("empty").ok()?;
    let given: Vec<&String> = given.collect();
    let Ok(mut iterator) = parameters.try_iter() else {
        return None;
    };
    let mut missing = None;
    while let Some(Ok(name)) = iterator.next() {
        let Ok(name) = name.extract::<String>() else {
            continue;
        };
        let Some(parameter) = parameters.get_item(&name).ok() else {
            continue;
        };
        let variadic = parameter
            .getattr("kind")
            .ok()
            .and_then(|kind| kind.getattr("name").ok())
            .and_then(|name| name.extract::<String>().ok())
            .is_some_and(|kind| kind == "VAR_POSITIONAL" || kind == "VAR_KEYWORD");
        if variadic {
            continue;
        }
        let has_default = parameter
            .getattr("default")
            .ok()
            .is_some_and(|default| !default.is(empty.clone()));
        if !has_default && missing.is_none() && !given.contains(&&name) {
            missing = Some(name);
        }
    }
    missing
}
