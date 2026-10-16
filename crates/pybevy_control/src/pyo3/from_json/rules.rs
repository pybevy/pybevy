use std::cell::OnceCell;

use bevy::reflect::enums::VariantInfo;
use pybevy_color::linear_rgba::PyLinearRgba;
use pybevy_core::public_error;
use pybevy_math::{quat::PyQuat, range::PyRange, vec3::PyVec3};
use pyo3::{
    prelude::*,
    types::{PyDict, PyFloat, PyInt, PyList, PyModule, PySet, PyString, PyTuple, PyType},
};
use serde_json::{Map, Value};

use crate::{
    codec::{
        enum_payload,
        json_float::nonfinite_float_from_json,
        math_shape::{MathShape, READBACK_ARRAYS, math_shape},
        reflect, value,
        value::{json_f32, json_f64},
    },
    pyo3::{
        enum_names,
        from_json::{
            color::{construct_color_variant, linear_rgba_from_json, srgba_from_array},
            enums::{
                VariantSource, construct_enum_variant, enum_owner_type, enum_variant_names,
                reject_enum_constant,
            },
            field::{
                construct_declared_optional, convert_field_value, entity_from_json,
                has_public_getset_fields, is_dataclass_instance, json_to_py, rebuild_array_like,
            },
            lists::convert_list_elements,
            math::{math_readback_array, math_value_from_json, range_from_json},
        },
    },
};

type Converted = Result<Option<Py<PyAny>>, String>;

/// A JSON value to convert for one field, and the field's current Python value.
pub(super) struct FieldContext<'a, 'py> {
    py: Python<'py>,
    component: &'a Bound<'py, PyAny>,
    field_name: &'a str,
    value: &'a Value,
    declared_type: Option<&'a str>,
    current: Bound<'py, PyAny>,
    type_name: String,
    owner: OnceCell<Bound<'py, PyType>>,
    is_pybevy_value: OnceCell<bool>,
}

impl<'a, 'py> FieldContext<'a, 'py> {
    pub(super) fn new(
        py: Python<'py>,
        component: &'a Bound<'py, PyAny>,
        field_name: &'a str,
        value: &'a Value,
        declared_type: Option<&'a str>,
        current: Bound<'py, PyAny>,
    ) -> Self {
        let type_name = current
            .get_type()
            .name()
            .map(|name| name.to_string())
            .unwrap_or_default();
        Self {
            py,
            component,
            field_name,
            value,
            declared_type,
            current,
            type_name,
            owner: OnceCell::new(),
            is_pybevy_value: OnceCell::new(),
        }
    }

    fn owner(&self) -> &Bound<'py, PyType> {
        self.owner.get_or_init(|| enum_owner_type(&self.current))
    }

    fn is_color(&self) -> bool {
        self.owner()
            .name()
            .is_ok_and(|name| name.to_string_lossy() == "Color")
    }

    fn is_pybevy_value(&self) -> bool {
        *self
            .is_pybevy_value
            .get_or_init(|| has_public_getset_fields(&self.current))
    }

    fn object(&self) -> Option<&'a Map<String, Value>> {
        self.value.as_object()
    }

    fn array(&self) -> Option<&'a Vec<Value>> {
        self.value.as_array()
    }

    fn single_entry(&self) -> Option<(&'a String, &'a Value)> {
        self.object()
            .filter(|fields| fields.len() == 1)
            .and_then(|fields| fields.iter().next())
    }
}

type Rule = fn(&FieldContext<'_, '_>) -> Converted;

/// Conversion rules in priority order; the first that applies decides the value.
pub(super) const RULES: &[Rule] = &[
    declared_optional,
    range,
    linear_rgba,
    vec3,
    quat,
    float,
    math_readback_object,
    math_shape_mismatch,
    opaque_repr,
    val,
    color_array,
    color_tagged_variant,
    color_variant_key,
    dataclass_patch,
    tagged_enum_variant,
    enum_variant_key,
    unit_enum_variant,
    unknown_enum_variant,
    entity,
    tuple,
    set,
    array_like,
    list_elements,
    math_array_length,
    math_array,
    wrapped_scalar,
    pybevy_value_patch,
];

fn declared_optional(cx: &FieldContext<'_, '_>) -> Converted {
    let Some(type_name) = cx.declared_type.filter(|_| cx.current.is_none()) else {
        return Ok(None);
    };
    construct_declared_optional(cx.py, type_name, cx.value)
}

fn range(cx: &FieldContext<'_, '_>) -> Converted {
    let Ok(range) = cx.current.cast_exact::<PyRange>() else {
        return Ok(None);
    };
    range_from_json(cx.py, range, cx.value).map(Some)
}

fn linear_rgba(cx: &FieldContext<'_, '_>) -> Converted {
    if cx.current.cast::<PyLinearRgba>().is_err() {
        return Ok(None);
    }
    linear_rgba_from_json(cx.py, &cx.current, cx.value).map(Some)
}

fn vec3(cx: &FieldContext<'_, '_>) -> Converted {
    if cx.current.cast_exact::<PyVec3>().is_err() {
        return Ok(None);
    }
    let value = value::vec3_from_json(cx.value)?;
    Py::new(cx.py, PyVec3::from_vec3(value))
        .map(|value| Some(value.into_any()))
        .map_err(|error| error.to_string())
}

fn quat(cx: &FieldContext<'_, '_>) -> Converted {
    if cx.current.cast_exact::<PyQuat>().is_err() {
        return Ok(None);
    }
    let value = value::unit_quat_from_json(cx.value)?;
    Py::new(cx.py, PyQuat::from_quat(value))
        .map(|value| Some(value.into_any()))
        .map_err(|error| error.to_string())
}

fn float(cx: &FieldContext<'_, '_>) -> Converted {
    if !cx.current.is_instance_of::<PyFloat>()
        || !(cx.value.is_number() || nonfinite_float_from_json(cx.value).is_some())
    {
        return Ok(None);
    }
    json_f64(cx.value)?
        .into_pyobject(cx.py)
        .map(|value| Some(value.into_any().unbind()))
        .map_err(|error| error.to_string())
}

fn math_readback_object(cx: &FieldContext<'_, '_>) -> Converted {
    let Some(Value::Array(values)) = math_readback_array(&cx.current, cx.value) else {
        return Ok(None);
    };
    math_value_from_json(cx.py, &cx.type_name, &values).transpose()
}

fn math_shape_mismatch(cx: &FieldContext<'_, '_>) -> Converted {
    let Some(shape) = math_shape(READBACK_ARRAYS, &cx.type_name).map(MathShape::label) else {
        return Ok(None);
    };
    let kind = match cx.value {
        Value::Array(_) => return Ok(None),
        other => value::json_kind_name(other),
    };
    Err(format!("{} expects {shape}, got {kind}", cx.type_name))
}

fn opaque_repr(cx: &FieldContext<'_, '_>) -> Converted {
    let Some((key, Value::String(expected))) = cx.single_entry() else {
        return Ok(None);
    };
    if key != "repr" {
        return Ok(None);
    }
    let actual = cx.current.repr().map_err(|error| error.to_string())?;
    if actual.to_string_lossy() == expected.as_str() {
        return Ok(Some(cx.current.clone().unbind()));
    }
    Err(format!(
        "opaque value cannot be edited; expected unchanged repr {actual}"
    ))
}

fn val(cx: &FieldContext<'_, '_>) -> Converted {
    if cx.type_name != "Val" {
        return Ok(None);
    }
    let Some((variant, payload)) = cx.single_entry() else {
        return Ok(None);
    };
    let method = match variant.as_str() {
        "Auto" => "auto",
        "Px" => "px",
        "Percent" => "percent",
        "Vw" => "vw",
        "Vh" => "vh",
        "VMin" => "vmin",
        "VMax" => "vmax",
        _ => {
            reject_enum_constant(cx.owner(), variant)?;
            return Err(format!("unknown Val variant '{variant}'"));
        }
    };
    let val_type = cx.current.get_type();
    if variant == "Auto" {
        return val_type
            .call_method0(method)
            .map(|value| Some(value.unbind()))
            .map_err(|error| error.to_string());
    }
    let value = json_f32(payload)?;
    val_type
        .call_method1(method, (value,))
        .map(|value| Some(value.unbind()))
        .map_err(|error| error.to_string())
}

fn color_array(cx: &FieldContext<'_, '_>) -> Converted {
    if !cx.is_color() {
        return Ok(None);
    }
    let Some(values) = cx.array().filter(|values| values.len() == 4) else {
        return Ok(None);
    };
    srgba_from_array(cx.owner(), values).map(Some)
}

fn color_tagged_variant(cx: &FieldContext<'_, '_>) -> Converted {
    if !cx.is_color() {
        return Ok(None);
    }
    let Some(fields) = cx.object() else {
        return Ok(None);
    };
    let (Some(Value::String(variant_name)), Some(variant_value)) =
        (fields.get("variant"), fields.get("value"))
    else {
        return Ok(None);
    };
    construct_color_variant(cx.py, variant_name, variant_value).map(Some)
}

fn color_variant_key(cx: &FieldContext<'_, '_>) -> Converted {
    if !cx.is_color() {
        return Ok(None);
    }
    let Some((variant_name, variant_value)) = cx.single_entry() else {
        return Ok(None);
    };
    if variant_name == "variant" {
        return Ok(None);
    }
    construct_color_variant(cx.py, variant_name, variant_value).map(Some)
}

/// Applies the given fields onto the current dataclass, so omitted fields
/// keep their value and nested ones convert recursively.
fn dataclass_patch(cx: &FieldContext<'_, '_>) -> Converted {
    let Some(fields) = cx.object() else {
        return Ok(None);
    };
    if !is_dataclass_instance(&cx.current) {
        return Ok(None);
    }
    let kwargs = PyDict::new(cx.py);
    for (name, value) in fields {
        if !cx.current.hasattr(name.as_str()).unwrap_or(false) {
            return Err(format!("{} has no field '{name}'", cx.type_name));
        }
        let converted = convert_field_value(cx.py, &cx.current, name, value)?;
        kwargs
            .set_item(name, converted)
            .map_err(|error| error.to_string())?;
    }
    let dataclasses = PyModule::import(cx.py, "dataclasses").map_err(|error| error.to_string())?;
    dataclasses
        .getattr("replace")
        .and_then(|replace| replace.call((&cx.current,), Some(&kwargs)))
        .map(|value| Some(value.unbind()))
        .map_err(|error| error.to_string())
}

/// The tagged form emitted by get_component, for every variant.
fn tagged_enum_variant(cx: &FieldContext<'_, '_>) -> Converted {
    let Some(obj) = cx.object() else {
        return Ok(None);
    };
    let Some((variant_name, payload)) = enum_payload::normalize(obj) else {
        return Ok(None);
    };
    if obj.contains_key("value") {
        let owner = cx.owner();
        let owner_name = owner.name().map_err(|error| error.to_string())?;
        let native_name = enum_names::canonical_variant_name(owner, variant_name);
        match reflect::compiled_enum_variant_info(&owner_name.to_string_lossy(), &native_name) {
            Some(VariantInfo::Struct(_)) => {
                return Err(format!(
                    "variant '{variant_name}' uses named payload fields; place them beside 'variant', not inside 'value'"
                ));
            }
            Some(VariantInfo::Unit(_)) => {
                return Err(format!(
                    "variant '{variant_name}' has no payload; omit 'value'"
                ));
            }
            _ => {}
        }
    }
    construct_enum_variant(
        cx.py,
        &cx.current,
        variant_name,
        &payload,
        VariantSource::Named,
    )
}

/// `{"Variant": value}` becomes `Type.Variant(value)`.
fn enum_variant_key(cx: &FieldContext<'_, '_>) -> Converted {
    let Some((variant_name, variant_value)) = cx.single_entry() else {
        return Ok(None);
    };
    if variant_name == "variant" {
        return Ok(None);
    }
    construct_enum_variant(
        cx.py,
        &cx.current,
        variant_name,
        variant_value,
        VariantSource::ObjectKey,
    )
}

fn unit_enum_variant(cx: &FieldContext<'_, '_>) -> Converted {
    let Value::String(variant_name) = cx.value else {
        return Ok(None);
    };
    construct_enum_variant(
        cx.py,
        &cx.current,
        variant_name,
        &Value::Null,
        VariantSource::Named,
    )
}

fn unknown_enum_variant(cx: &FieldContext<'_, '_>) -> Converted {
    let Value::String(name) = cx.value else {
        return Ok(None);
    };
    let owner = cx.owner();
    let variants = enum_variant_names(owner);
    if variants.is_empty() {
        return Ok(None);
    }
    let owner_name = owner
        .name()
        .map_err(|error| error.to_string())?
        .to_string_lossy()
        .into_owned();
    Err(format!(
        "unknown variant '{name}' for {}. Valid variants: {}",
        owner_name,
        enum_names::accepted_variant_names(owner, variants).join(", ")
    ))
}

/// JSON has no entity type, so an Entity-valued field takes an id.
fn entity(cx: &FieldContext<'_, '_>) -> Converted {
    if cx.type_name != "Entity" {
        return Ok(None);
    }
    entity_from_json(cx.py, cx.value).map(Some)
}

fn tuple(cx: &FieldContext<'_, '_>) -> Converted {
    let Some(values) = cx.array() else {
        return Ok(None);
    };
    if !cx.current.is_instance_of::<PyTuple>() {
        return Ok(None);
    }
    let values = values
        .iter()
        .map(|value| json_to_py(cx.py, value))
        .collect::<Result<Vec<_>, _>>()?;
    PyTuple::new(cx.py, values)
        .map(|value| Some(value.into_any().unbind()))
        .map_err(|error| error.to_string())
}

fn set(cx: &FieldContext<'_, '_>) -> Converted {
    let Some(values) = cx.array() else {
        return Ok(None);
    };
    if !cx.current.is_instance_of::<PySet>() {
        return Ok(None);
    }
    let values = values
        .iter()
        .map(|value| json_to_py(cx.py, value))
        .collect::<Result<Vec<_>, _>>()?;
    PySet::new(cx.py, values)
        .map(|value| Some(value.into_any().unbind()))
        .map_err(|error| error.to_string())
}

fn array_like(cx: &FieldContext<'_, '_>) -> Converted {
    if !cx.value.is_array() {
        return Ok(None);
    }
    rebuild_array_like(cx.py, &cx.current, cx.value)
}

fn list_elements(cx: &FieldContext<'_, '_>) -> Converted {
    let Some(values) = cx.array() else {
        return Ok(None);
    };
    if !cx.current.is_instance_of::<PyList>() || values.is_empty() {
        return Ok(None);
    }
    convert_list_elements(cx.py, cx.component, cx.field_name, &cx.current, values)
}

fn math_array_length(cx: &FieldContext<'_, '_>) -> Converted {
    let Some(values) = cx.array() else {
        return Ok(None);
    };
    let Some(shape) = math_shape(READBACK_ARRAYS, &cx.type_name) else {
        return Ok(None);
    };
    if values.len() == shape.coordinates.len() {
        return Ok(None);
    }
    Err(format!(
        "{} expects {}, got {} elements",
        cx.type_name,
        shape.label(),
        values.len()
    ))
}

fn math_array(cx: &FieldContext<'_, '_>) -> Converted {
    let Some(values) = cx.array() else {
        return Ok(None);
    };
    if cx.type_name == "Vec3" {
        return Ok(None);
    }
    math_value_from_json(cx.py, &cx.type_name, values).transpose()
}

/// A single-value pybevy wrapper built from its scalar payload.
fn wrapped_scalar(cx: &FieldContext<'_, '_>) -> Converted {
    if !cx.is_pybevy_value()
        || cx.current.hasattr("variant").unwrap_or(false)
        || !cx.current.hasattr("value").unwrap_or(false)
        || cx.value.is_object()
    {
        return Ok(None);
    }
    let value = json_to_py(cx.py, cx.value)?;
    cx.current
        .get_type()
        .call1((value,))
        .map(|value| Some(value.unbind()))
        .map_err(|error| error.to_string())
}

fn pybevy_value_patch(cx: &FieldContext<'_, '_>) -> Converted {
    if !cx.is_pybevy_value() {
        return Ok(None);
    }
    let Some(fields) = cx.object() else {
        return Ok(None);
    };
    // Scalar getset descriptors must not turn object patches into default values.
    if cx.current.is_instance_of::<PyFloat>()
        || cx.current.is_instance_of::<PyInt>()
        || cx.current.is_instance_of::<PyString>()
    {
        return Err(public_error::mcp_scalar_object(&cx.type_name));
    }
    if fields.is_empty() {
        return Ok(Some(cx.current.clone().unbind()));
    }
    let kwargs = PyDict::new(cx.py);
    for (name, value) in fields {
        if !cx.current.hasattr(name.as_str()).unwrap_or(false) {
            return Err(format!(
                "{} has no field '{name}'",
                cx.current
                    .get_type()
                    .name()
                    .map_err(|error| error.to_string())?
            ));
        }
        let converted = convert_field_value(cx.py, &cx.current, name, value)?;
        kwargs
            .set_item(name, converted)
            .map_err(|error| error.to_string())?;
    }
    cx.current
        .get_type()
        .call((), Some(&kwargs))
        .map(|value| Some(value.unbind()))
        .map_err(|error| error.to_string())
}
