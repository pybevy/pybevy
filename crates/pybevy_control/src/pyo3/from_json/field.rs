use std::{any::TypeId, collections::HashMap, sync::Arc};

use bevy::{
    ecs::{reflect::AppTypeRegistry, world::World},
    reflect::TypeInfo,
};
use pybevy_core::{ComponentBridge, PyEntity, public_error};
use pyo3::{
    prelude::*,
    types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyModule, PyTuple, PyType},
};

use crate::pyo3::{
    dataclass_fields::dataclass_field_names,
    descriptors::is_getset_descriptor,
    from_json::{
        color::srgba_from_array,
        enums::{
            EnumVariantRequest, VariantSource, construct_enum_variant_for_owner,
            enum_component_variant_request, enum_field_prototype, enum_owner_type,
            enum_variant_names, tagged_enum_variant_request,
        },
        lists::find_pybevy_class,
        math::math_readback_array,
        rules::{FieldContext, RULES},
    },
};

/// Return whether `field_name` appears in the class's declared component
/// fields. `None` means the class exposes no supported declaration metadata,
/// so callers may fall back to instance attribute lookup.
pub(crate) fn class_declares_component_field(
    class: &Bound<'_, PyType>,
    field_name: &str,
) -> Option<bool> {
    if let Ok(Some(names)) = dataclass_field_names(class) {
        return Some(names.contains(field_name));
    }
    let mut found_metadata = false;
    for attribute in ["__dataclass_fields__", "__annotations__"] {
        let Ok(metadata) = class.getattr(attribute) else {
            continue;
        };
        let Ok(fields) = metadata.cast::<PyDict>() else {
            continue;
        };
        found_metadata = true;
        if fields.contains(field_name).unwrap_or(false) {
            return Some(true);
        }
    }
    found_metadata.then_some(false)
}

pub(crate) fn has_public_instance_field(instance: &Bound<'_, PyAny>, field_name: &str) -> bool {
    !field_name.starts_with('_')
        && instance
            .getattr("__dict__")
            .ok()
            .and_then(|fields| fields.cast_into::<PyDict>().ok())
            .is_some_and(|fields| fields.contains(field_name).unwrap_or(false))
}

fn resolved_field_annotation<'py>(
    py: Python<'py>,
    class: &Bound<'py, PyType>,
    field_name: &str,
) -> Option<Bound<'py, PyAny>> {
    let typing = PyModule::import(py, "typing").ok()?;
    let hints = typing
        .call_method1("get_type_hints", (class,))
        .ok()?
        .cast_into::<PyDict>()
        .ok()?;
    hints.get_item(field_name).ok().flatten()
}

fn annotation_name(annotation: &Bound<'_, PyAny>) -> String {
    // `float | None` and `list[int]` name themselves only through str().
    if annotation.hasattr("__args__").unwrap_or(false) {
        return annotation.to_string();
    }
    annotation
        .getattr("__name__")
        .and_then(|name| name.extract::<String>())
        .unwrap_or_else(|_| annotation.to_string())
}

fn annotation_enum_type<'py>(
    py: Python<'py>,
    annotation: &Bound<'py, PyAny>,
) -> Result<Option<(Bound<'py, PyType>, bool)>, String> {
    let typing = PyModule::import(py, "typing").map_err(|error| error.to_string())?;
    let origin = typing
        .call_method1("get_origin", (annotation,))
        .map_err(|error| error.to_string())?;
    let types = PyModule::import(py, "types").map_err(|error| error.to_string())?;
    let is_union = origin.is(typing.getattr("Union").map_err(|error| error.to_string())?)
        || origin.is(types
            .getattr("UnionType")
            .map_err(|error| error.to_string())?);
    let mut enum_type = annotation.cast::<PyType>().ok().cloned();
    let mut optional = false;
    if is_union {
        let args = typing
            .call_method1("get_args", (annotation,))
            .map_err(|error| error.to_string())?
            .cast_into::<PyTuple>()
            .map_err(|error| error.to_string())?;
        if args.len() == 2 {
            for index in 0..2 {
                let arg = args.get_item(index).map_err(|error| error.to_string())?;
                if arg.is(py.None().bind(py).get_type()) {
                    optional = true;
                } else {
                    enum_type = arg.cast_into::<PyType>().ok();
                }
            }
        }
        if !optional {
            enum_type = None;
        }
    }
    Ok(enum_type
        .filter(|owner| !enum_variant_names(owner).is_empty())
        .map(|owner| (owner, optional)))
}

fn convert_declared_enum_field_value(
    py: Python<'_>,
    instance: &Bound<'_, PyAny>,
    field_name: &str,
    field_value: &serde_json::Value,
) -> Result<Option<Py<PyAny>>, String> {
    let Some(annotation) = resolved_field_annotation(py, &instance.get_type(), field_name) else {
        return Ok(None);
    };
    let Some((owner, optional)) = annotation_enum_type(py, &annotation)? else {
        return Ok(None);
    };
    let request = match field_value {
        serde_json::Value::String(name) => Some(EnumVariantRequest {
            name: name.clone(),
            payload: serde_json::Value::Null,
            source: VariantSource::Named,
        }),
        serde_json::Value::Object(fields) if fields.contains_key("variant") => {
            let Ok(current) = instance.getattr(field_name) else {
                return Ok(None);
            };
            let current_owner = enum_owner_type(&current);
            if current_owner.is(&owner) {
                return Ok(None);
            }
            if enum_variant_names(&current_owner).is_empty() {
                tagged_enum_variant_request(fields)
            } else {
                enum_component_variant_request(&current, fields)
            }
        }
        _ => None,
    };
    let Some(request) = request else {
        return Ok(None);
    };
    construct_enum_variant_for_owner(
        py,
        owner.as_any(),
        &owner,
        &request.name,
        &request.payload,
        request.source,
    )
    .map_err(|error| {
        if optional {
            public_error::mcp_optional_enum_error(&error)
        } else {
            error
        }
    })
}

pub(crate) fn validate_annotated_field_value(
    py: Python<'_>,
    class: &Bound<'_, PyType>,
    field_name: &str,
    mut value: Py<PyAny>,
) -> Result<Py<PyAny>, String> {
    let Some(annotation) = resolved_field_annotation(py, class, field_name) else {
        return Ok(value);
    };

    if let Ok(name) = value.bind(py).extract::<String>()
        && let Some((owner, optional)) = annotation_enum_type(py, &annotation)?
    {
        let converted = construct_enum_variant_for_owner(
            py,
            owner.as_any(),
            &owner,
            &name,
            &serde_json::Value::Null,
            VariantSource::Named,
        )
        .map_err(|error| {
            if optional {
                public_error::mcp_optional_enum_error(&error)
            } else {
                error
            }
        })?;
        if let Some(converted) = converted {
            value = converted;
        }
    }

    if annotation.is(py.get_type::<PyFloat>())
        && value.bind(py).is_instance_of::<PyInt>()
        && !value.bind(py).is_instance_of::<PyBool>()
    {
        value = py
            .get_type::<PyFloat>()
            .call1((value.bind(py),))
            .map_err(|error| error.to_string())?
            .unbind();
    }

    let builtins = PyModule::import(py, "builtins").map_err(|error| error.to_string())?;
    let direct_check = builtins
        .getattr("isinstance")
        .and_then(|isinstance| isinstance.call1((value.bind(py), &annotation)))
        .and_then(|result| result.extract::<bool>());
    let compatible = match direct_check {
        Ok(compatible) => compatible,
        Err(_) => {
            let typing = PyModule::import(py, "typing").map_err(|error| error.to_string())?;
            let origin = typing
                .call_method1("get_origin", (&annotation,))
                .map_err(|error| error.to_string())?;
            if origin.is_none() {
                return Ok(value);
            }
            builtins
                .getattr("isinstance")
                .and_then(|isinstance| isinstance.call1((value.bind(py), origin)))
                .and_then(|result| result.extract::<bool>())
                .map_err(|error| error.to_string())?
        }
    };

    if compatible {
        return Ok(value);
    }

    let actual = value
        .bind(py)
        .get_type()
        .name()
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .into_owned();
    Err(format!(
        "expected {}, got {actual}",
        annotation_name(&annotation)
    ))
}

pub(crate) fn convert_annotated_field_value(
    py: Python<'_>,
    instance: &Bound<'_, PyAny>,
    field_name: &str,
    field_value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    if let Some(value) = convert_declared_enum_field_value(py, instance, field_name, field_value)? {
        return Ok(value);
    }
    let value = convert_field_value(py, instance, field_name, field_value)?;
    validate_annotated_field_value(py, &instance.get_type(), field_name, value)
}

pub(crate) fn convert_committed_resource_enum_field_value(
    py: Python<'_>,
    instance: &Bound<'_, PyAny>,
    field_name: &str,
    field_value: &serde_json::Value,
) -> Result<Option<Py<PyAny>>, String> {
    let Ok(validation) = instance
        .get_type()
        .getattr("__pybevy_resource_assignment_validation__")
    else {
        return Ok(None);
    };
    let current = validation
        .call_method1("_committed_enum_type", (field_name,))
        .map_err(|error| error.to_string())?;
    if current.is_none() {
        return Ok(None);
    }
    let owner = current
        .cast_into::<PyType>()
        .map_err(|error| error.to_string())?;
    let Some(annotation) = resolved_field_annotation(py, &instance.get_type(), field_name) else {
        return Ok(None);
    };
    let Some((_, optional)) = annotation_enum_type(py, &annotation)? else {
        return Ok(None);
    };
    let request = if let Some(name) = field_value.as_str() {
        Some(EnumVariantRequest {
            name: name.to_string(),
            payload: serde_json::Value::Null,
            source: VariantSource::Named,
        })
    } else if let Some(fields) = field_value.as_object()
        && fields.contains_key("variant")
    {
        let current = instance
            .getattr(field_name)
            .map_err(|error| error.to_string())?;
        enum_component_variant_request(&current, fields)
    } else {
        None
    };
    let Some(request) = request else {
        return Ok(None);
    };
    construct_enum_variant_for_owner(
        py,
        owner.as_any(),
        &owner,
        &request.name,
        &request.payload,
        request.source,
    )
    .map_err(|error| {
        if optional {
            public_error::mcp_optional_enum_error(&error)
        } else {
            error
        }
    })
}

/// Build an `Entity` from the id form used throughout the control API.
pub(crate) fn entity_from_json(
    py: Python<'_>,
    value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    let bits = value
        .as_u64()
        .ok_or_else(|| format!("expected an entity id, got {value}"))?;
    let entity = PyEntity::from_bits(bits).map_err(|error| error.to_string())?;
    Ok(Py::new(py, entity)
        .map_err(|error| error.to_string())?
        .into_any())
}

/// Convert a JSON value for a constructor argument.
///
/// Same as [`json_to_py`] except on a relationship component's entity field,
/// where JSON's integer is an entity id rather than a plain number.
pub(crate) fn json_to_py_for_field(
    py: Python<'_>,
    bridge: &Arc<dyn ComponentBridge>,
    field_name: &str,
    value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    if bridge.relationship_field() == Some(field_name) {
        return entity_from_json(py, value);
    }
    json_to_py(py, value)
}

pub(crate) fn optional_field_types(world: &World, type_id: TypeId) -> HashMap<String, String> {
    let mut types = HashMap::new();
    let Some(app_registry) = world.get_resource::<AppTypeRegistry>() else {
        return types;
    };
    let registry = app_registry.read();
    let Some(registration) = registry.get(type_id) else {
        return types;
    };
    let TypeInfo::Struct(info) = registration.type_info() else {
        return types;
    };
    for index in 0..info.field_len() {
        let Some(field) = info.field_at(index) else {
            continue;
        };
        let Some(inner) = field
            .type_path_table()
            .short_path()
            .strip_prefix("Option<")
            .and_then(|path| path.strip_suffix('>'))
        else {
            continue;
        };
        types.insert(field.name().to_string(), inner.to_string());
    }
    types
}

pub(crate) fn declared_optional_type<'a>(
    types: &'a HashMap<String, String>,
    field_name: &str,
) -> Option<&'a str> {
    types
        .get(field_name)
        .or_else(|| types.get(field_name.strip_suffix('_')?))
        .map(String::as_str)
}

pub(super) fn construct_declared_optional(
    py: Python<'_>,
    type_name: &str,
    field_value: &serde_json::Value,
) -> Result<Option<Py<PyAny>>, String> {
    let Some(class) = find_pybevy_class(py, type_name) else {
        return Ok(None);
    };
    if type_name == "Color"
        && let serde_json::Value::Array(values) = field_value
    {
        return srgba_from_array(&class, values).map(Some);
    }
    let serde_json::Value::Object(fields) = field_value else {
        return Ok(None);
    };
    let template = class
        .call0()
        .map_err(|error| format!("{type_name} has no default constructor: {error}"))?;
    let kwargs = PyDict::new(py);
    for (name, value) in fields {
        if !template.hasattr(name.as_str()).unwrap_or(false) {
            return Err(format!("{type_name} has no field '{name}'"));
        }
        let current = template
            .getattr(name.as_str())
            .map_err(|error| error.to_string())?;
        let normalized = math_readback_array(&current, value);
        let converted =
            convert_field_value(py, &template, name, normalized.as_ref().unwrap_or(value))?;
        kwargs
            .set_item(name, converted)
            .map_err(|error| error.to_string())?;
    }
    class
        .call((), Some(&kwargs))
        .map(Bound::unbind)
        .map(Some)
        .map_err(|error| error.to_string())
}

/// Whether the value is a dataclass instance, the same check `dataclasses`
/// itself performs, without importing the module for every candidate field.
pub(super) fn is_dataclass_instance(value: &Bound<'_, PyAny>) -> bool {
    !value.is_instance_of::<PyType>() && value.hasattr("__dataclass_fields__").unwrap_or(false)
}

/// Rebuild an array-like field (numpy and friends) from a JSON array through
/// the module that owns the current value, so no array dependency is linked in.
pub(super) fn rebuild_array_like(
    py: Python<'_>,
    current: &Bound<'_, PyAny>,
    field_value: &serde_json::Value,
) -> Result<Option<Py<PyAny>>, String> {
    if !current.hasattr("__array__").unwrap_or(false) {
        return Ok(None);
    }
    let Ok(module_name) = current
        .get_type()
        .getattr("__module__")
        .and_then(|name| name.extract::<String>())
    else {
        return Ok(None);
    };
    let Ok(module) = PyModule::import(py, module_name.as_str()) else {
        return Ok(None);
    };
    let Ok(as_array) = module.getattr("asarray") else {
        return Ok(None);
    };
    let values = json_to_py(py, field_value)?;
    let kwargs = PyDict::new(py);
    if let Ok(dtype) = current.getattr("dtype") {
        kwargs
            .set_item("dtype", dtype)
            .map_err(|error| error.to_string())?;
    }
    as_array
        .call((values,), Some(&kwargs))
        .map(|array| Some(array.unbind()))
        .map_err(|error| error.to_string())
}

pub(super) fn has_public_getset_fields(value: &Bound<'_, PyAny>) -> bool {
    value.get_type().dir().is_ok_and(|names| {
        names.iter().any(|name| {
            let Ok(name) = name.extract::<String>() else {
                return false;
            };
            !name.starts_with('_')
                && value
                    .get_type()
                    .getattr(name.as_str())
                    .is_ok_and(|attribute| is_getset_descriptor(&attribute))
        })
    })
}

pub(crate) fn convert_field_value(
    py: Python<'_>,
    component: &Bound<'_, PyAny>,
    field_name: &str,
    field_value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    convert_field_value_typed(py, component, field_name, field_value, None)
}

pub(crate) fn convert_field_value_typed(
    py: Python<'_>,
    component: &Bound<'_, PyAny>,
    field_name: &str,
    field_value: &serde_json::Value,
    declared_type: Option<&str>,
) -> Result<Py<PyAny>, String> {
    if let Ok(current) = component.getattr(field_name) {
        let current = if let Ok(variant) = component.cast::<PyType>() {
            enum_field_prototype(py, variant, field_name)?.unwrap_or(current)
        } else {
            current
        };
        let context = FieldContext::new(
            py,
            component,
            field_name,
            field_value,
            declared_type,
            current,
        );
        for rule in RULES {
            if let Some(value) = rule(&context)? {
                return Ok(value);
            }
        }
    }
    json_to_py(py, field_value)
}

/// Convert a JSON value to a Python object
pub(crate) fn json_to_py(py: Python<'_>, value: &serde_json::Value) -> Result<Py<PyAny>, String> {
    match value {
        serde_json::Value::Null => Ok(py.None()),
        serde_json::Value::Bool(b) => Ok(b
            .into_pyobject(py)
            .map_err(|e| e.to_string())?
            .to_owned()
            .into_any()
            .unbind()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.into_pyobject(py)
                    .map_err(|e| e.to_string())?
                    .to_owned()
                    .into_any()
                    .unbind())
            } else if let Some(u) = n.as_u64() {
                Ok(u.into_pyobject(py)
                    .map_err(|e| e.to_string())?
                    .to_owned()
                    .into_any()
                    .unbind())
            } else if let Some(f) = n.as_f64() {
                Ok(f.into_pyobject(py)
                    .map_err(|e| e.to_string())?
                    .to_owned()
                    .into_any()
                    .unbind())
            } else {
                Err("Unsupported number type".into())
            }
        }
        serde_json::Value::String(s) => Ok(s
            .into_pyobject(py)
            .map_err(|e| e.to_string())?
            .into_any()
            .unbind()),
        serde_json::Value::Array(arr) => {
            let list = PyList::empty(py);
            for item in arr {
                let py_item = json_to_py(py, item)?;
                list.append(py_item).map_err(|e| e.to_string())?;
            }
            Ok(list.into_any().unbind())
        }
        serde_json::Value::Object(obj) => {
            let dict = PyDict::new(py);
            for (k, v) in obj {
                let py_value = json_to_py(py, v)?;
                dict.set_item(k, py_value).map_err(|e| e.to_string())?;
            }
            Ok(dict.into_any().unbind())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::CString, sync::Once};

    use bevy::math::Vec3;
    use pybevy_color::linear_rgba::PyLinearRgba;
    use pybevy_math::vec3::PyVec3;
    use pyo3::types::PySet;

    use super::*;

    static INIT: Once = Once::new();

    fn setup_python() {
        INIT.call_once(|| {
            Python::initialize();
        });
    }

    #[test]
    fn convert_field_value_color_array() {
        setup_python();
        Python::attach(|py| {
            // Create a mock Color class with a srgba static method
            let code = CString::new(
                r#"
import types, sys

class Color:
    def __init__(self, r=1.0, g=1.0, b=1.0, a=1.0):
        self.r = r
        self.g = g
        self.b = b
        self.a = a

    @staticmethod
    def srgba(r, g, b, a):
        return Color(r, g, b, a)

# Register as pybevy.color in sys.modules
color_mod = types.ModuleType("pybevy.color")
color_mod.Color = Color

# Ensure parent package exists in sys.modules too
if "pybevy" not in sys.modules:
    pybevy_mod = types.ModuleType("pybevy")
    sys.modules["pybevy"] = pybevy_mod
sys.modules["pybevy.color"] = color_mod

# Create a holder object with a color attribute of type Color
class Holder:
    def __init__(self):
        self.color = Color()

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();

            let holder = globals.get_item("holder").unwrap().unwrap();

            // Call convert_field_value with a JSON array [1.0, 0.0, 0.0, 1.0]
            let field_value = serde_json::json!([1.0, 0.0, 0.0, 1.0]);
            let result = convert_field_value(py, &holder, "color", &field_value);
            assert!(result.is_ok(), "convert_field_value failed: {:?}", result);

            let py_obj = result.unwrap();
            let bound = py_obj.bind(py);

            // Verify it's a Color instance with correct values
            let type_name = bound.get_type().name().unwrap().to_string();
            assert_eq!(type_name, "Color");
            let r: f64 = bound.getattr("r").unwrap().extract().unwrap();
            let g: f64 = bound.getattr("g").unwrap().extract().unwrap();
            let b: f64 = bound.getattr("b").unwrap().extract().unwrap();
            let a: f64 = bound.getattr("a").unwrap().extract().unwrap();
            assert!((r - 1.0).abs() < 1e-10);
            assert!((g - 0.0).abs() < 1e-10);
            assert!((b - 0.0).abs() < 1e-10);
            assert!((a - 1.0).abs() < 1e-10);
        });
    }

    #[test]
    fn convert_field_value_color_wrong_array_length() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import types, sys

class Color:
    def __init__(self, r=1.0, g=1.0, b=1.0, a=1.0):
        self.r = r
        self.g = g
        self.b = b
        self.a = a

    @staticmethod
    def srgba(r, g, b, a):
        return Color(r, g, b, a)

color_mod = types.ModuleType("pybevy.color")
color_mod.Color = Color

if "pybevy" not in sys.modules:
    pybevy_mod = types.ModuleType("pybevy")
    sys.modules["pybevy"] = pybevy_mod
sys.modules["pybevy.color"] = color_mod
sys.modules.setdefault("pybevy.math", types.ModuleType("pybevy.math"))

class Holder:
    def __init__(self):
        self.color = Color()

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();

            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!([1.0, 0.0, 0.0]);
            let error = convert_field_value(py, &holder, "color", &field_value).unwrap_err();
            assert_eq!(error, "Color expects [r, g, b, a], got 3 elements");
        });
    }

    #[test]
    fn convert_field_value_linear_rgba_accepts_array_and_object_forms() {
        setup_python();
        Python::attach(|py| {
            let holder = PyModule::from_code(
                py,
                c"class Holder: pass\nholder = Holder()\n",
                c"linear_rgba_holder.py",
                c"linear_rgba_holder",
            )
            .unwrap()
            .getattr("holder")
            .unwrap();
            holder
                .setattr(
                    "emissive",
                    Py::new(py, PyLinearRgba::new(0.0, 0.0, 0.0, 1.0)).unwrap(),
                )
                .unwrap();

            for (input, expected) in [
                (
                    serde_json::json!([6.5, 3.0, 1.0, 1.0]),
                    [6.5, 3.0, 1.0, 1.0],
                ),
                (
                    serde_json::json!({
                        "red": 0.25,
                        "green": 0.5,
                        "blue": 0.75,
                        "alpha": 0.9,
                    }),
                    [0.25, 0.5, 0.75, 0.9],
                ),
            ] {
                let converted = convert_field_value(py, &holder, "emissive", &input).unwrap();
                let value: PyRef<'_, PyLinearRgba> = converted.bind(py).extract().unwrap();
                assert_eq!(value.to_f32_array().unwrap(), expected);
            }
        });
    }

    #[test]
    fn convert_field_value_linear_rgba_reports_accepted_forms() {
        setup_python();
        Python::attach(|py| {
            let holder = PyModule::from_code(
                py,
                c"class Holder: pass\nholder = Holder()\n",
                c"linear_rgba_errors.py",
                c"linear_rgba_errors",
            )
            .unwrap()
            .getattr("holder")
            .unwrap();
            holder
                .setattr(
                    "emissive",
                    Py::new(py, PyLinearRgba::new(0.0, 0.0, 0.0, 1.0)).unwrap(),
                )
                .unwrap();

            for (input, detail) in [
                (serde_json::json!([1.0, 2.0, 3.0]), "got 3 elements"),
                (
                    serde_json::json!([1.0, "green", 3.0, 1.0]),
                    "LinearRgba.green",
                ),
                (serde_json::json!("WHITE"), "LinearRgba got string"),
                (
                    serde_json::json!({"LinearRgba": {"red": 1.0}}),
                    "LinearRgba has no field 'LinearRgba'",
                ),
            ] {
                let error = convert_field_value(py, &holder, "emissive", &input).unwrap_err();
                assert!(error.contains(detail), "unexpected error: {error}");
                assert!(error.contains(public_error::MCP_LINEAR_RGBA_INPUT_FORMS));
                assert!(!error.contains("Valid variants"));
                assert!(!error.contains("BLACK"));
            }
        });
    }

    #[test]
    fn convert_field_value_unrelated_linear_rgba_class_falls_through() {
        setup_python();
        Python::attach(|py| {
            let holder = PyModule::from_code(
                py,
                c"class LinearRgba:\n    pass\nclass Holder:\n    emissive = LinearRgba()\nholder = Holder()\n",
                c"custom_linear_rgba.py",
                c"custom_linear_rgba",
            )
            .unwrap()
            .getattr("holder")
            .unwrap();

            let converted = convert_field_value(
                py,
                &holder,
                "emissive",
                &serde_json::json!([1.0, 2.0, 3.0, 4.0]),
            )
            .unwrap();

            assert_eq!(
                converted.bind(py).extract::<Vec<f64>>().unwrap(),
                vec![1.0, 2.0, 3.0, 4.0]
            );
        });
    }

    #[test]
    fn convert_field_value_non_color_falls_through() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Holder:
    def __init__(self):
        self.name = "hello"

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();

            let holder = globals.get_item("holder").unwrap().unwrap();

            // String field with a string value should fall through to generic conversion
            let field_value = serde_json::json!("world");
            let result = convert_field_value(py, &holder, "name", &field_value);
            assert!(result.is_ok());
            let py_obj = result.unwrap();
            let bound = py_obj.bind(py);
            let val: String = bound.extract().unwrap();
            assert_eq!(val, "world");
        });
    }

    #[test]
    fn convert_field_value_nested_dataclass_keeps_omitted_fields() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import dataclasses

@dataclasses.dataclass
class Stats:
    hp: int = 100
    mp: int = 50

class Holder:
    def __init__(self):
        self.stats = Stats()

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!({"hp": 10});
            let result = convert_field_value(py, &holder, "stats", &field_value).unwrap();
            let bound = result.bind(py);

            assert_eq!(bound.get_type().name().unwrap().to_string(), "Stats");
            assert_eq!(bound.getattr("hp").unwrap().extract::<i64>().unwrap(), 10);
            assert_eq!(bound.getattr("mp").unwrap().extract::<i64>().unwrap(), 50);
        });
    }

    #[test]
    fn convert_field_value_set_from_array() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Holder:
    def __init__(self):
        self.tags = {"a"}

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!(["x", "y"]);
            let result = convert_field_value(py, &holder, "tags", &field_value).unwrap();
            let bound = result.bind(py);

            assert!(bound.is_instance_of::<PySet>());
            assert_eq!(bound.len().unwrap(), 2);
        });
    }

    #[test]
    fn convert_field_value_quat_reports_expected_shape() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Quat:
    pass

class Holder:
    def __init__(self):
        self.rotation = Quat()

holder = Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();
            let field_value = serde_json::json!([0.1, 0.2, 0.3]);

            let error = convert_field_value(py, &holder, "rotation", &field_value).unwrap_err();

            assert_eq!(error, "Quat expects [x, y, z, w], got 3 elements");
        });
    }

    #[test]
    fn convert_field_value_vec3_string_reports_expected_shape() {
        setup_python();
        Python::attach(|py| {
            let holder = PyModule::from_code(
                py,
                c"class Holder: pass\nholder = Holder()\n",
                c"vec3_holder.py",
                c"vec3_holder",
            )
            .unwrap()
            .getattr("holder")
            .unwrap();
            holder
                .setattr(
                    "translation",
                    Py::new(py, PyVec3::from_vec3(Vec3::ZERO)).unwrap(),
                )
                .unwrap();

            let error =
                convert_field_value(py, &holder, "translation", &serde_json::json!("not_a_vec3"))
                    .unwrap_err();

            assert_eq!(
                error,
                "Vec3 got string; accepted forms: [x, y, z] or {\"x\": x, \"y\": y, \"z\": z}, with finite numeric coordinates"
            );
        });
    }

    #[test]
    fn convert_field_value_native_vec3_accepts_array_and_object_forms() {
        setup_python();
        Python::attach(|py| {
            let holder = PyModule::from_code(
                py,
                c"class Holder: pass\nholder = Holder()\n",
                c"vec3_holder.py",
                c"vec3_holder",
            )
            .unwrap()
            .getattr("holder")
            .unwrap();
            holder
                .setattr(
                    "vector",
                    Py::new(py, PyVec3::from_vec3(Vec3::ZERO)).unwrap(),
                )
                .unwrap();

            for input in [
                serde_json::json!([1.25, -2.5, 3.75]),
                serde_json::json!({"x": 1.25, "y": -2.5, "z": 3.75}),
            ] {
                let converted = convert_field_value(py, &holder, "vector", &input).unwrap();
                let wrapped = converted.bind(py).extract::<PyVec3>().unwrap();
                assert_eq!(
                    Vec3::try_from(wrapped).unwrap(),
                    Vec3::new(1.25, -2.5, 3.75)
                );
            }
        });
    }

    #[test]
    fn convert_field_value_unrelated_vec3_class_keeps_generic_conversion() {
        setup_python();
        Python::attach(|py| {
            let holder = PyModule::from_code(
                py,
                c"import dataclasses\n@dataclasses.dataclass\nclass Vec3:\n    x: float = 0.0\n    y: float = 0.0\n    z: float = 0.0\nclass Holder:\n    vector = Vec3()\nholder = Holder()\n",
                c"custom_vec3.py",
                c"custom_vec3",
            )
            .unwrap()
            .getattr("holder")
            .unwrap();

            let converted = convert_field_value(
                py,
                &holder,
                "vector",
                &serde_json::json!({"x": 4.0, "y": 5.0, "z": 6.0}),
            )
            .unwrap();
            let value = converted.bind(py);

            assert!(value.cast_exact::<PyVec3>().is_err());
            assert_eq!(value.get_type().module().unwrap(), "custom_vec3");
            assert_eq!(value.getattr("x").unwrap().extract::<f64>().unwrap(), 4.0);
            assert_eq!(value.getattr("y").unwrap().extract::<f64>().unwrap(), 5.0);
            assert_eq!(value.getattr("z").unwrap().extract::<f64>().unwrap(), 6.0);
        });
    }

    #[test]
    fn convert_field_value_list_of_struct_elements_converts_dicts() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import sys, types

if "pybevy" not in sys.modules:
    sys.modules["pybevy"] = types.ModuleType("pybevy")
ui_mod = sys.modules.get("pybevy.ui")
if ui_mod is None:
    ui_mod = types.ModuleType("pybevy.ui")
    sys.modules["pybevy.ui"] = ui_mod

class ShadowStyle:
    color = None
    x_offset = None
    def __init__(self, *, color=None, x_offset=None):
        self.color = color
        self.x_offset = x_offset

ShadowStyle.__module__ = "pybevy.ui"
ui_mod.ShadowStyle = ShadowStyle

class BoxHolder:
    def __init__(self):
        self._shadows = [ShadowStyle()]
    @property
    def shadows(self):
        return self._shadows
    @shadows.setter
    def shadows(self, value):
        for item in value:
            if not isinstance(item, ShadowStyle):
                raise TypeError("'dict' object is not an instance of 'ShadowStyle'")
        self._shadows = list(value)

holder = BoxHolder()
holder_empty = BoxHolder.__new__(BoxHolder)
holder_empty._shadows = []
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!([
                {"color": [1.0, 0.5], "x_offset": 3.0},
                {"color": null, "x_offset": 1.5}
            ]);
            let result = convert_field_value(py, &holder, "shadows", &field_value).unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 2);
            let first = list.get_item(0).unwrap();
            assert_eq!(
                first.get_type().name().unwrap().to_string_lossy(),
                "ShadowStyle"
            );
            assert_eq!(
                first.getattr("x_offset").unwrap().extract::<f64>().unwrap(),
                3.0
            );
            let second = list.get_item(1).unwrap();
            assert!(second.getattr("color").unwrap().is_none());
            assert_eq!(
                second
                    .getattr("x_offset")
                    .unwrap()
                    .extract::<f64>()
                    .unwrap(),
                1.5
            );
        });
    }

    #[test]
    fn convert_field_value_list_of_structs_empty_list_probes_detached_setter() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import sys, types

if "pybevy" not in sys.modules:
    sys.modules["pybevy"] = types.ModuleType("pybevy")
ui_mod = sys.modules.get("pybevy.ui")
if ui_mod is None:
    ui_mod = types.ModuleType("pybevy.ui")
    sys.modules["pybevy.ui"] = ui_mod

class ShadowStyle:
    color = None
    x_offset = None
    def __init__(self, *, color=None, x_offset=None):
        self.color = color
        self.x_offset = x_offset

ShadowStyle.__module__ = "pybevy.ui"
ui_mod.ShadowStyle = ShadowStyle

class BoxHolder:
    def __init__(self):
        self._shadows = []
        self.setter_attempts = 0
    @property
    def shadows(self):
        return self._shadows
    @shadows.setter
    def shadows(self, value):
        self.setter_attempts += 1
        for item in value:
            if not isinstance(item, ShadowStyle):
                raise TypeError("'dict' object is not an instance of 'ShadowStyle'")
        self._shadows = list(value)

holder_empty = BoxHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder_empty = globals.get_item("holder_empty").unwrap().unwrap();

            let field_value = serde_json::json!([{"color": null, "x_offset": 7.0}]);
            let result = convert_field_value(py, &holder_empty, "shadows", &field_value).unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 1);
            assert_eq!(
                list.get_item(0)
                    .unwrap()
                    .getattr("x_offset")
                    .unwrap()
                    .extract::<f64>()
                    .unwrap(),
                7.0
            );
            let shadows = holder_empty.getattr("shadows").unwrap();
            assert_eq!(shadows.len().unwrap(), 0);
            assert_eq!(
                holder_empty
                    .getattr("setter_attempts")
                    .unwrap()
                    .extract::<usize>()
                    .unwrap(),
                0
            );
        });
    }

    #[test]
    fn convert_field_value_empty_list_probe_unresolvable_class_falls_back_generic() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class UnresolvableHolder:
    def __init__(self):
        self._items = []
    @property
    def items(self):
        return self._items
    @items.setter
    def items(self, value):
        for item in value:
            if type(item) is dict:
                raise TypeError("'dict' object is not an instance of 'UnloadedThing'")
        self._items = list(value)

holder = UnresolvableHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!([{"a": 1}]);
            let result = convert_field_value(py, &holder, "items", &field_value).unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 1);
            let element = list.get_item(0).unwrap();
            assert!(element.is_instance_of::<PyDict>());
            let items = holder.getattr("items").unwrap();
            assert_eq!(items.len().unwrap(), 0);
        });
    }

    #[test]
    fn convert_field_value_list_element_unknown_field() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import sys, types

if "pybevy" not in sys.modules:
    sys.modules["pybevy"] = types.ModuleType("pybevy")
ui_mod = sys.modules.get("pybevy.ui")
if ui_mod is None:
    ui_mod = types.ModuleType("pybevy.ui")
    sys.modules["pybevy.ui"] = ui_mod

class ShadowStyle:
    color = None
    x_offset = None
    def __init__(self, *, color=None, x_offset=None):
        self.color = color
        self.x_offset = x_offset

ShadowStyle.__module__ = "pybevy.ui"
ui_mod.ShadowStyle = ShadowStyle

class BoxHolder:
    def __init__(self):
        self._shadows = [ShadowStyle()]
    @property
    def shadows(self):
        return self._shadows
    @shadows.setter
    def shadows(self, value):
        for item in value:
            if not isinstance(item, ShadowStyle):
                raise TypeError("'dict' object is not an instance of 'ShadowStyle'")
        self._shadows = list(value)

holder = BoxHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!([{"bogus": 1.0}]);
            let error = convert_field_value(py, &holder, "shadows", &field_value).unwrap_err();
            assert_eq!(error, "element 0: ShadowStyle has no field 'bogus'");
        });
    }

    #[test]
    fn convert_field_value_list_element_wrong_kind() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class ShadowStyle:
    def __init__(self):
        pass

class BoxHolder:
    def __init__(self):
        self._shadows = [ShadowStyle()]
    @property
    def shadows(self):
        return self._shadows
    @shadows.setter
    def shadows(self, value):
        for item in value:
            if not isinstance(item, ShadowStyle):
                raise TypeError("'dict' object is not an instance of 'ShadowStyle'")
        self._shadows = list(value)

holder = BoxHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let holder = globals.get_item("holder").unwrap().unwrap();

            let field_value = serde_json::json!(["just a string"]);
            let error = convert_field_value(py, &holder, "shadows", &field_value).unwrap_err();
            assert_eq!(error, "element 0: expected object, got string");
        });
    }

    #[test]
    fn convert_field_value_list_element_missing_constructor_field() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import sys, types

if "pybevy" not in sys.modules:
    sys.modules["pybevy"] = types.ModuleType("pybevy")
ui_mod = sys.modules.get("pybevy.ui")
if ui_mod is None:
    ui_mod = types.ModuleType("pybevy.ui")
    sys.modules["pybevy.ui"] = ui_mod

class HalfSpace:
    normal_d = None
    def __init__(self, normal_d):
        self.normal_d = normal_d

HalfSpace.__module__ = "pybevy.ui"
ui_mod.HalfSpace = HalfSpace

class FrustumHolder:
    def __init__(self):
        self._half_spaces = []
    @property
    def half_spaces(self):
        return self._half_spaces
    @half_spaces.setter
    def half_spaces(self, value):
        for item in value:
            if not isinstance(item, HalfSpace):
                raise TypeError("'dict' object is not an instance of 'HalfSpace'")
        self._half_spaces = list(value)

frustum = FrustumHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let frustum = globals.get_item("frustum").unwrap().unwrap();

            let error = convert_field_value(py, &frustum, "half_spaces", &serde_json::json!([{}]))
                .unwrap_err();
            assert_eq!(error, "element 0: HalfSpace requires field 'normal_d'");

            let field_value = serde_json::json!([{"normal_d": [1.0, 2.0, 3.0, 4.0]}]);
            let result = convert_field_value(py, &frustum, "half_spaces", &field_value).unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 1);
            let element = list.get_item(0).unwrap();
            assert_eq!(
                element.get_type().name().unwrap().to_string_lossy(),
                "HalfSpace"
            );
            let normal_d = element.getattr("normal_d").unwrap();
            assert_eq!(normal_d.get_item(0).unwrap().extract::<f64>().unwrap(), 1.0);
        });
    }

    #[test]
    fn convert_field_value_list_of_scalars_keeps_generic_path() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class TagHolder:
    def __init__(self):
        self.tags = ["a"]

tag_holder = TagHolder()
tag_holder_empty = TagHolder()
tag_holder_empty.tags = []
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let tag_holder = globals.get_item("tag_holder").unwrap().unwrap();
            let tag_holder_empty = globals.get_item("tag_holder_empty").unwrap().unwrap();

            let result =
                convert_field_value(py, &tag_holder, "tags", &serde_json::json!(["x", "y"]))
                    .unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 2);
            assert_eq!(list.get_item(0).unwrap().extract::<String>().unwrap(), "x");

            let result =
                convert_field_value(py, &tag_holder_empty, "tags", &serde_json::json!(["x"]))
                    .unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 1);
            let tags = tag_holder_empty.getattr("tags").unwrap();
            assert_eq!(tags.len().unwrap(), 0);
        });
    }

    #[test]
    fn convert_field_value_list_of_math_elements() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
import sys, types

if "pybevy" not in sys.modules:
    sys.modules["pybevy"] = types.ModuleType("pybevy")
math_mod = sys.modules.get("pybevy.math")
if math_mod is None:
    math_mod = types.ModuleType("pybevy.math")
    sys.modules["pybevy.math"] = math_mod

class Vec3:
    def __init__(self, x, y, z):
        self.x = x
        self.y = y
        self.z = z

Vec3.__module__ = "pybevy.math"
math_mod.Vec3 = Vec3

class Vec3Holder:
    def __init__(self):
        self._points = [Vec3(0.0, 0.0, 0.0)]
    @property
    def points(self):
        return self._points
    @points.setter
    def points(self, value):
        for item in value:
            if not isinstance(item, Vec3):
                raise TypeError("'dict' object is not an instance of 'Vec3'")
        self._points = list(value)

vec3_holder = Vec3Holder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let vec3_holder = globals.get_item("vec3_holder").unwrap().unwrap();

            let result =
                convert_field_value(py, &vec3_holder, "points", &serde_json::json!([[1, 2, 3]]))
                    .unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            let element = list.get_item(0).unwrap();
            assert_eq!(element.get_type().name().unwrap().to_string_lossy(), "Vec3");
            assert_eq!(element.getattr("x").unwrap().extract::<f64>().unwrap(), 1.0);
            assert_eq!(element.getattr("z").unwrap().extract::<f64>().unwrap(), 3.0);

            let error =
                convert_field_value(py, &vec3_holder, "points", &serde_json::json!(["bad"]))
                    .unwrap_err();
            assert_eq!(error, "element 0: Vec3 expects [x, y, z], got string");

            let error =
                convert_field_value(py, &vec3_holder, "points", &serde_json::json!([[1.0, 2.0]]))
                    .unwrap_err();
            assert_eq!(error, "element 0: Vec3 expects [x, y, z], got 2 elements");
        });
    }

    #[test]
    fn convert_field_value_list_of_enum_elements() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Track:
    pass

class Auto(Track):
    def __init__(self):
        pass

class Px(Track):
    def __init__(self, value):
        self.value = value

Track.Auto = Auto
Track.Px = Px

class TrackHolder:
    def __init__(self):
        self._tracks = [Track.Px(1.0)]
    @property
    def tracks(self):
        return self._tracks
    @tracks.setter
    def tracks(self, value):
        for item in value:
            if not isinstance(item, Track):
                raise TypeError("'dict' object is not an instance of 'Track'")
        self._tracks = list(value)

track_holder = TrackHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let track_holder = globals.get_item("track_holder").unwrap().unwrap();

            let field_value = serde_json::json!([
                {"Px": 5.0},
                {"Auto": null},
                {"variant": "Px", "value": 2.5}
            ]);
            let result = convert_field_value(py, &track_holder, "tracks", &field_value).unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            assert_eq!(list.len(), 3);
            let first = list.get_item(0).unwrap();
            assert_eq!(first.get_type().name().unwrap().to_string_lossy(), "Px");
            assert_eq!(
                first.getattr("value").unwrap().extract::<f64>().unwrap(),
                5.0
            );
            let second = list.get_item(1).unwrap();
            assert_eq!(second.get_type().name().unwrap().to_string_lossy(), "Auto");
            let third = list.get_item(2).unwrap();
            assert_eq!(
                third.getattr("value").unwrap().extract::<f64>().unwrap(),
                2.5
            );

            let error = convert_field_value(
                py,
                &track_holder,
                "tracks",
                &serde_json::json!([{"Bogus": 1.0}]),
            )
            .unwrap_err();
            assert_eq!(
                error,
                "element 0: unknown variant 'Bogus' for Track. Valid variants: Auto, Px"
            );
        });
    }

    #[test]
    fn convert_field_value_list_enum_element_missing_field() {
        setup_python();
        Python::attach(|py| {
            let code = CString::new(
                r#"
class Track:
    pass

class Box(Track):
    def __init__(self, x, y):
        self.x = x
        self.y = y

Track.Box = Box

class TrackHolder:
    def __init__(self):
        self._tracks = [Track.Box(1.0, 2.0)]
    @property
    def tracks(self):
        return self._tracks
    @tracks.setter
    def tracks(self, value):
        for item in value:
            if not isinstance(item, Track):
                raise TypeError("'dict' object is not an instance of 'Track'")
        self._tracks = list(value)

track_holder = TrackHolder()
"#,
            )
            .unwrap();

            let globals = PyDict::new(py);
            py.run(&code, Some(&globals), None).unwrap();
            let track_holder = globals.get_item("track_holder").unwrap().unwrap();

            let error = convert_field_value(
                py,
                &track_holder,
                "tracks",
                &serde_json::json!([{"Box": {"x": 1.0}}]),
            )
            .unwrap_err();
            assert_eq!(error, "element 0: variant 'Box' requires field 'y'");

            let result = convert_field_value(
                py,
                &track_holder,
                "tracks",
                &serde_json::json!([{"variant": "Box", "value": {"x": 1.0, "y": 2.0}}]),
            )
            .unwrap();
            let list = result.bind(py).cast::<PyList>().unwrap();
            let element = list.get_item(0).unwrap();
            assert_eq!(element.get_type().name().unwrap().to_string_lossy(), "Box");
            assert_eq!(element.getattr("y").unwrap().extract::<f64>().unwrap(), 2.0);
        });
    }
}
