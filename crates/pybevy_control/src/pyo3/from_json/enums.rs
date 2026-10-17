use bevy::reflect::enums::VariantInfo;
use pybevy_core::public_error;
use pyo3::{
    prelude::*,
    types::{PyBool, PyDict, PyFloat, PyInt, PyModule, PyString, PyType},
};

use crate::{
    codec::{enum_payload, reflect},
    pyo3::{
        enum_names,
        from_json::{
            field::{convert_field_value, json_to_py},
            lists::{constructible_representative, find_pybevy_class},
        },
        to_json::py_value_to_json,
    },
};

pub(crate) fn enum_owner_type<'py>(current: &Bound<'py, PyAny>) -> Bound<'py, PyType> {
    let current_type = current.get_type();
    let Some(base) = current_type
        .getattr("__bases__")
        .ok()
        .and_then(|bases| bases.get_item(0).ok())
        .and_then(|base| base.cast_into::<PyType>().ok())
    else {
        return current_type;
    };
    let nested_variant = base.dir().is_ok_and(|names| {
        names.iter().any(|name| {
            let Ok(name) = name.extract::<String>() else {
                return false;
            };
            base.getattr(name.as_str())
                .ok()
                .and_then(|candidate| candidate.cast_into::<PyType>().ok())
                .is_some_and(|candidate| candidate.is(&current_type))
        })
    });
    if nested_variant { base } else { current_type }
}

#[derive(Default)]
struct EnumMembers {
    variants: Vec<String>,
    constants: Vec<String>,
}

pub(crate) fn same_type_class_attributes(owner: &Bound<'_, PyType>) -> (Vec<String>, Vec<String>) {
    let mut instances = Vec::new();
    let mut classes = Vec::new();
    for name in owner
        .dir()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|name| name.extract::<String>().ok())
        .filter(|name| !name.starts_with('_'))
    {
        let Ok(value) = owner.getattr(name.as_str()) else {
            continue;
        };
        if value.is_instance(owner).unwrap_or(false) {
            instances.push(name);
        } else if value
            .cast::<PyType>()
            .is_ok_and(|variant| variant.is_subclass(owner).unwrap_or(false) && !variant.is(owner))
        {
            classes.push(name);
        }
    }
    (instances, classes)
}

fn enum_members(owner: &Bound<'_, PyType>) -> EnumMembers {
    let (instances, mut variants) = same_type_class_attributes(owner);
    let has_variant_classes = !variants.is_empty();
    if !has_variant_classes && !enum_names::is_enum_type(owner) {
        return EnumMembers::default();
    }
    let (unit_variants, mut constants) =
        enum_names::partition_unit_variants(owner, instances, has_variant_classes);
    variants.extend(unit_variants);
    variants.sort();
    constants.sort();
    EnumMembers {
        variants,
        constants,
    }
}

pub(crate) fn enum_variant_names(owner: &Bound<'_, PyType>) -> Vec<String> {
    enum_members(owner).variants
}

pub(crate) fn reject_enum_constant(owner: &Bound<'_, PyType>, name: &str) -> Result<(), String> {
    let EnumMembers {
        variants,
        constants,
    } = enum_members(owner);
    if !constants.iter().any(|constant| constant == name) {
        return Ok(());
    }
    let owner_name = owner.name().map_err(|error| error.to_string())?;
    let written = owner
        .getattr(name)
        .ok()
        .map(|value| py_value_to_json(&value))
        .filter(|value| value.get("variant").is_some())
        .map(|value| inline_json(&value));
    Err(public_error::mcp_enum_constant_not_variant(
        name,
        &owner_name.to_string_lossy(),
        written.as_deref(),
        &enum_names::accepted_variant_names(owner, variants),
    ))
}

pub(crate) fn inline_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(fields) => {
            let entries = fields
                .get_key_value("variant")
                .into_iter()
                .chain(fields.iter().filter(|(name, _)| *name != "variant"))
                .map(|(name, value)| {
                    format!(
                        "{}: {}",
                        serde_json::Value::from(name.as_str()),
                        inline_json(value)
                    )
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", entries.join(", "))
        }
        serde_json::Value::Array(items) => format!(
            "[{}]",
            items.iter().map(inline_json).collect::<Vec<_>>().join(", ")
        ),
        other => other.to_string(),
    }
}

pub(crate) fn enum_field_prototype<'py>(
    py: Python<'py>,
    variant: &Bound<'py, PyType>,
    field_name: &str,
) -> Result<Option<Bound<'py, PyAny>>, String> {
    let bases = variant
        .getattr("__bases__")
        .map_err(|error| error.to_string())?;
    let owner = bases
        .get_item(0)
        .map_err(|error| error.to_string())?
        .cast_into::<PyType>()
        .map_err(|error| error.to_string())?;
    let owner_name = owner.name().map_err(|error| error.to_string())?;
    let variant_name = variant.name().map_err(|error| error.to_string())?;
    let native_variant =
        enum_names::canonical_variant_name(&owner, &variant_name.to_string_lossy());
    let Some(VariantInfo::Struct(fields)) =
        reflect::compiled_enum_variant_info(&owner_name.to_string_lossy(), &native_variant)
    else {
        return Ok(None);
    };
    let Some(type_info) = fields.field(field_name).and_then(|field| field.type_info()) else {
        return Ok(None);
    };
    let Some(type_name) = type_info.type_path_table().ident() else {
        return Ok(None);
    };
    let constant = match type_name {
        "Vec2" | "Vec3" | "Vec4" => "ZERO",
        "Quat" => "IDENTITY",
        _ => return Ok(None),
    };
    let math = PyModule::import(py, "pybevy.math").map_err(|error| error.to_string())?;
    math.getattr(type_name)
        .and_then(|class| class.getattr(constant))
        .map(Some)
        .map_err(|error| error.to_string())
}

// Distinguishes explicit names from untagged object keys for error reporting.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VariantSource {
    Named,
    ObjectKey,
}

pub(crate) struct EnumVariantRequest {
    pub(crate) name: String,
    pub(crate) payload: serde_json::Value,
    pub(crate) source: VariantSource,
}

pub(crate) fn enum_component_variant_request(
    current: &Bound<'_, PyAny>,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Option<EnumVariantRequest> {
    if let Some(request) = tagged_enum_variant_request(fields) {
        return Some(request);
    }

    let (name, payload) = fields.iter().next().filter(|_| fields.len() == 1)?;
    let owner = enum_owner_type(current);
    enum_names::accepted_variant_names(&owner, enum_variant_names(&owner))
        .iter()
        .any(|variant| variant == name)
        .then(|| EnumVariantRequest {
            name: name.clone(),
            payload: payload.clone(),
            source: VariantSource::ObjectKey,
        })
}

pub(crate) fn tagged_enum_variant_request(
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Option<EnumVariantRequest> {
    let (name, payload) = enum_payload::normalize(fields)?;
    Some(EnumVariantRequest {
        name: name.to_owned(),
        payload,
        source: VariantSource::Named,
    })
}

fn variant_payload_type<'py>(
    py: Python<'py>,
    owner: &Bound<'py, PyType>,
    variant: &Bound<'py, PyAny>,
    native_name: &str,
) -> Option<Bound<'py, PyType>> {
    if let Ok(prototype) = variant.getattr("__pybevy_control_payload_prototype__") {
        return prototype.cast_into::<PyType>().ok();
    }
    let owner_name = owner.name().ok()?;
    let Some(VariantInfo::Tuple(payload)) =
        reflect::compiled_enum_variant_info(&owner_name.to_string_lossy(), native_name)
    else {
        return None;
    };
    if payload.field_len() != 1 {
        return None;
    }
    find_pybevy_class(py, payload.field_at(0)?.type_path_table().ident()?)
}

fn construct_wrapped_variant_payload<'py>(
    py: Python<'py>,
    variant: &Bound<'py, PyAny>,
    payload_type: &Bound<'py, PyType>,
    variant_name: &str,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<Py<PyAny>>, String> {
    let Some(prototype) = constructible_representative(payload_type) else {
        return Ok(None);
    };
    let holder = PyModule::import(py, "types")
        .and_then(|types| types.getattr("SimpleNamespace"))
        .and_then(|ns| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("value", prototype.clone())?;
            ns.call((), Some(&kwargs))
        })
        .map_err(|error| format!("invalid payload for variant '{variant_name}': {error}"))?;
    let payload = convert_field_value(
        py,
        &holder,
        "value",
        &serde_json::Value::Object(fields.clone()),
    )?;
    variant
        .call1((payload,))
        .map(|result| Some(result.unbind()))
        .map_err(|error| format!("invalid payload for variant '{variant_name}': {error}"))
}

pub(crate) fn construct_enum_variant(
    py: Python<'_>,
    current: &Bound<'_, PyAny>,
    variant_name: &str,
    variant_value: &serde_json::Value,
    source: VariantSource,
) -> Result<Option<Py<PyAny>>, String> {
    let owner = enum_owner_type(current);
    construct_enum_variant_for_owner(py, current, &owner, variant_name, variant_value, source)
}

pub(crate) fn construct_enum_variant_for_owner(
    py: Python<'_>,
    current: &Bound<'_, PyAny>,
    owner: &Bound<'_, PyType>,
    variant_name: &str,
    variant_value: &serde_json::Value,
    source: VariantSource,
) -> Result<Option<Py<PyAny>>, String> {
    let variants = enum_variant_names(owner);
    let python_name = enum_names::python_variant_name(owner, variant_name);
    let native_name = enum_names::canonical_variant_name(owner, &python_name);
    // Payload fields are attributes too; accept only registered variant names.
    let variant = owner
        .getattr(python_name.as_str())
        .ok()
        .filter(|_| variants.iter().any(|name| name == &python_name));
    let Some(variant) = variant else {
        if variants.is_empty() {
            return Ok(None);
        }
        let owner_name = owner
            .name()
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .into_owned();
        reject_enum_constant(owner, variant_name)?;
        let valid_variants = enum_names::accepted_variant_names(owner, variants);
        if variant_name == "value" && source == VariantSource::ObjectKey {
            return Err(format!(
                "a payload needs the variant it belongs to: pass \
                 {{\"variant\": \"<name>\", \"value\": ...}} for {owner_name}. \
                 Valid variants: {}",
                valid_variants.join(", ")
            ));
        }
        return Err(public_error::mcp_unknown_variant(
            variant_name,
            &owner_name,
            &valid_variants,
        ));
    };

    let is_unit = variant_value.is_null()
        || variant_value
            .as_object()
            .is_some_and(serde_json::Map::is_empty);
    if is_unit {
        if let Ok(result) = variant.call0() {
            return Ok(Some(result.unbind()));
        }
        if variant.is_instance(owner).unwrap_or(false) {
            return Ok(Some(variant.unbind()));
        }
        return Err(format!("variant '{variant_name}' requires a payload"));
    }

    let has_wrapped_payload = variant
        .hasattr("__pybevy_control_payload_prototype__")
        .unwrap_or(false);
    if variant_value.is_array() {
        let expected = if has_wrapped_payload {
            public_error::MCP_ENUM_WRAPPED_ARRAY_PAYLOAD
        } else {
            public_error::MCP_ENUM_SCALAR_ARRAY_PAYLOAD
        };
        return Err(format!(
            "invalid payload for variant '{variant_name}': {expected}"
        ));
    }

    if variant
        .cast::<PyType>()
        .is_ok_and(|variant_type| current.is_instance(variant_type).unwrap_or(false))
        && current.hasattr("value").unwrap_or(false)
    {
        let scalar_payload = current.getattr("value").is_ok_and(|payload| {
            payload.is_instance_of::<PyBool>()
                || payload.is_instance_of::<PyFloat>()
                || payload.is_instance_of::<PyInt>()
                || payload.is_instance_of::<PyString>()
        });
        if variant_value.is_object() && !has_wrapped_payload && scalar_payload {
            return Err(format!(
                "invalid payload for variant '{variant_name}': {}",
                public_error::MCP_ENUM_SCALAR_OBJECT_PAYLOAD
            ));
        }
        let argument = convert_field_value(py, current, "value", variant_value)?;
        return variant
            .call1((argument,))
            .map(|result| Some(result.unbind()))
            .map_err(|error| format!("invalid payload for variant '{variant_name}': {error}"));
    }

    if let serde_json::Value::Object(fields) = variant_value {
        let kwargs = PyDict::new(py);
        for (name, value) in fields {
            let value = convert_field_value(py, &variant, name, value)
                .map_err(|error| format!("{name}: {error}"))?;
            kwargs
                .set_item(name, value)
                .map_err(|error| error.to_string())?;
        }
        match variant.call((), Some(&kwargs)) {
            Ok(result) => return Ok(Some(result.unbind())),
            Err(error)
                if owner.name().is_ok_and(|name| {
                    matches!(
                        reflect::compiled_enum_variant_info(&name.to_string_lossy(), &native_name),
                        Some(VariantInfo::Struct(_))
                    )
                }) =>
            {
                return Err(format!(
                    "invalid payload for variant '{variant_name}': {error}"
                ));
            }
            Err(_) => {
                // A failed kwargs call may mean the variant wraps one payload value.
                if let Some(payload_type) = variant_payload_type(py, owner, &variant, &native_name)
                    && let Some(result) = construct_wrapped_variant_payload(
                        py,
                        &variant,
                        &payload_type,
                        variant_name,
                        fields,
                    )?
                {
                    return Ok(Some(result));
                }
            }
        }
    }

    let argument = json_to_py(py, variant_value)?;
    match variant.call1((argument,)) {
        Ok(result) => Ok(Some(result.unbind())),
        Err(_) if variant_value.is_object() => Err(format!(
            "invalid payload for variant '{variant_name}': {}",
            public_error::MCP_ENUM_SCALAR_OBJECT_PAYLOAD
        )),
        Err(error) => Err(format!(
            "invalid payload for variant '{variant_name}': {error}"
        )),
    }
}

// An enum has no variant field to setattr; resolve the tagged payload via the shared resolver.
pub(crate) fn resolve_enum_instance(
    py: Python<'_>,
    default: &Bound<'_, PyAny>,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<Py<PyAny>>, String> {
    let Some((variant_name, payload)) = enum_payload::normalize(fields) else {
        return Ok(None);
    };
    construct_enum_variant(py, default, variant_name, &payload, VariantSource::Named)
}

#[cfg(test)]
mod tests {
    use std::{ffi::CString, sync::Once};

    use super::*;
    use crate::test_python::initialize_python;

    static INIT: Once = Once::new();

    #[pyclass]
    struct ControlPayloadFixture {
        required: i64,
        preserved: i64,
    }

    #[pymethods]
    impl ControlPayloadFixture {
        #[new]
        #[pyo3(signature = (*, required = 1, preserved = 9))]
        fn new(required: i64, preserved: i64) -> Self {
            Self {
                required,
                preserved,
            }
        }

        #[getter]
        fn required(&self) -> i64 {
            self.required
        }

        #[getter]
        fn preserved(&self) -> i64 {
            self.preserved
        }
    }

    fn setup_python() {
        INIT.call_once(|| {
            initialize_python();
        });
    }

    #[test]
    fn wrapped_enum_payload_uses_registered_payload_type_defaults() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Variant:
    __pybevy_control_payload_prototype__ = Payload

    def __init__(self, value):
        if not isinstance(value, Payload):
            raise TypeError("expected Payload")
        self.value = value

variant = Variant
"#,
            )
            .unwrap();
            let globals = PyDict::new(py);
            globals
                .set_item("Payload", py.get_type::<ControlPayloadFixture>())
                .unwrap();
            py.run(&code, Some(&globals), None).unwrap();
            let variant = globals.get_item("variant").unwrap().unwrap();
            let payload_type = py.get_type::<ControlPayloadFixture>();
            let fields = serde_json::json!({"required": 4});

            let result = construct_wrapped_variant_payload(
                py,
                &variant,
                &payload_type,
                "Wrapped",
                fields.as_object().unwrap(),
            )
            .unwrap()
            .expect("metadata should construct a wrapped payload");
            let payload = result.bind(py).getattr("value").unwrap();

            assert_eq!(
                payload
                    .getattr("required")
                    .unwrap()
                    .extract::<i64>()
                    .unwrap(),
                4
            );
            assert_eq!(
                payload
                    .getattr("preserved")
                    .unwrap()
                    .extract::<i64>()
                    .unwrap(),
                9
            );
        });
    }
}
