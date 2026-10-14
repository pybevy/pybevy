use std::alloc::Layout;

use bevy::ecs::{component::ComponentId, world::EntityRef as BevyEntityRef};
use pybevy_color::{color::PyColor, linear_rgba::PyLinearRgba};
use pybevy_core::{PyHandle, public_error};
use pybevy_transform::global_transform::PyGlobalTransform;
#[cfg(test)]
use pyo3::ffi;
use pyo3::{
    prelude::*,
    types::{PyCFunction, PyDict, PyFloat, PyInt, PyModule, PyType},
};

use super::enum_names::{canonical_variant_name, is_enum_type};
use crate::{
    codec::{
        json_float::float_to_json,
        math_shape::{FLOAT_VECTORS, math_shape},
        value::{EnumPayload, LINEAR_RGBA_CHANNELS, enum_to_json},
    },
    pyo3::{
        custom,
        descriptors::{accepts_own_value, is_getset_descriptor},
        from_json::enums::enum_variant_names,
    },
};

/// Extract field values from a custom component.
/// Returns a JSON map of field names to JSON-converted values.
pub(crate) fn extract_custom_component_fields(
    py: Python<'_>,
    entity_ref: &BevyEntityRef,
    component_id: ComponentId,
    entry: &pybevy_core::CustomComponentEntry,
    descriptor_layout: Layout,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let ptr = entity_ref.get_by_id(component_id).ok()?;

    if !entry.is_pyobject_storage {
        let layout = entry.wrapper_layout.as_deref()?;
        if !custom::descriptor_matches(layout, descriptor_layout) {
            return None;
        }
        return custom::fields_to_json(ptr, layout).ok();
    }

    // SAFETY: CustomComponentInfo records this ComponentId as PyObject storage,
    // whose registered descriptor stores a live Py<PyAny> at this pointer.
    let py_obj: &Py<PyAny> = unsafe { &*(ptr.as_ptr() as *const Py<PyAny>) };
    let bound = py_obj.bind(py);

    python_object_fields_to_json(py, bound)
}

/// Public fields of a Python-stored value: its dataclass fields, else its
/// public `__dict__` entries; `None` when it has neither.
pub(crate) fn python_object_fields_to_json(
    py: Python<'_>,
    bound: &Bound<'_, PyAny>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    if let Some(fields) = dataclass_fields_to_json(py, bound)
        && !fields.is_empty()
    {
        return Some(fields);
    }
    let dict = bound.getattr("__dict__").ok()?;
    let fields: serde_json::Map<String, serde_json::Value> = dict
        .cast::<PyDict>()
        .ok()?
        .iter()
        .filter_map(|(key, value)| {
            let key = key.extract::<String>().ok()?;
            (!key.starts_with('_')).then(|| (key, py_value_to_json(&value)))
        })
        .collect();
    (!fields.is_empty()).then_some(fields)
}

/// Extract the fields of a dataclass instance as JSON.
/// Returns `None` when the value is not a dataclass instance.
pub(crate) fn dataclass_fields_to_json(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let mut context = SerializationContext {
        active: vec![value.as_ptr() as usize],
    };
    dataclass_fields_to_json_inner(py, value, &mut context)
}

fn dataclass_fields_to_json_inner(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    context: &mut SerializationContext,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    if value.is_instance_of::<PyType>() || !value.hasattr("__dataclass_fields__").unwrap_or(false) {
        return None;
    }
    let fields = py
        .import("dataclasses")
        .and_then(|dataclasses| dataclasses.getattr("fields"))
        .and_then(|fields| fields.call1((value,)))
        .ok()?;
    let mut map = serde_json::Map::new();
    for field in fields.try_iter().ok()?.flatten() {
        let name_value = field
            .getattr("name")
            .and_then(|name| name.extract::<String>())
            .ok()
            .and_then(|name| value.getattr(name.as_str()).ok().map(|value| (name, value)));
        if let Some((name, value)) = name_value {
            map.insert(name, py_value_to_json_inner(&value, context));
        }
    }
    Some(map)
}

const SERIALIZATION_MAX_DEPTH: usize = 64;

#[derive(Default)]
struct SerializationContext {
    // Ancestor Bounds keep these addresses alive during the traversal.
    active: Vec<usize>,
}

/// Convert Python values to JSON with bounded nesting and ancestor-cycle detection.
pub(crate) fn py_value_to_json(value: &Bound<'_, PyAny>) -> serde_json::Value {
    py_value_to_json_inner(value, &mut SerializationContext::default())
}

fn py_value_to_json_inner(
    value: &Bound<'_, PyAny>,
    context: &mut SerializationContext,
) -> serde_json::Value {
    let address = value.as_ptr() as usize;
    if context.active.contains(&address) {
        return serde_json::json!({"serialization_error": public_error::CONTROL_SERIALIZATION_CYCLE});
    }
    if context.active.len() >= SERIALIZATION_MAX_DEPTH {
        return serde_json::json!({"serialization_error": public_error::CONTROL_SERIALIZATION_DEPTH});
    }
    context.active.push(address);
    let result = serialize_python_value(value, context);
    context.active.pop();
    result
}

fn serialize_python_value(
    value: &Bound<'_, PyAny>,
    context: &mut SerializationContext,
) -> serde_json::Value {
    // None
    if value.is_none() {
        return serde_json::Value::Null;
    }
    // bool (must check before int, since bool is a subclass of int in Python)
    if let Ok(b) = value.extract::<bool>() {
        return serde_json::Value::Bool(b);
    }
    // int
    if let Ok(i) = value.extract::<i64>() {
        return serde_json::json!(i);
    }
    // float
    if let Ok(f) = value.extract::<f64>() {
        return float_to_json(f);
    }
    // str
    if let Ok(s) = value.extract::<String>() {
        return serde_json::Value::String(s);
    }
    if let Some(color) = color_to_json(value, context) {
        return color;
    }
    if let Some(linear_rgba) = linear_rgba_to_json(value) {
        return linear_rgba;
    }
    if let Some(vector) = vector_to_json(value) {
        return vector;
    }
    if let Some(entity) = entity_to_json(value) {
        return entity;
    }
    if let Some(handle) = handle_to_json(value) {
        return handle;
    }
    if let Some(member) = python_enum_to_json(value) {
        return member;
    }
    if pyo3_complex_enum_variant_name(&value.get_type()).is_some() {
        return serde_json::Value::Object(extract_bridge_fields_with_context(value, context));
    }
    // Preserve mapping entries before generic iteration can discard their values.
    if let Ok(dict) = value.cast::<PyDict>() {
        let mut map = serde_json::Map::new();
        let mut pairs = Vec::with_capacity(dict.len());
        let mut object_keys = true;
        for (key, item) in dict.iter() {
            let item = py_value_to_json_inner(&item, context);
            pairs.push(serde_json::Value::Array(vec![
                py_value_to_json_inner(&key, context),
                item.clone(),
            ]));
            if let Some(key) = json_object_key(&key).filter(|key| !map.contains_key(key)) {
                map.insert(key, item);
            } else {
                object_keys = false;
            }
        }
        return if object_keys {
            serde_json::Value::Object(map)
        } else {
            serde_json::Value::Array(pairs)
        };
    }
    if let Some(fields) = dataclass_fields_to_json_inner(value.py(), value, context) {
        return serde_json::Value::Object(fields);
    }
    // Serialize iterable elements.
    if let Ok(iter) = value.try_iter() {
        let elements: Vec<serde_json::Value> = iter
            .filter_map(|item| item.ok())
            .map(|item| py_value_to_json_inner(&item, context))
            .collect();
        return serde_json::Value::Array(elements);
    }
    // Check if this is a nested PyO3 struct (has getset_descriptor attributes)
    let nested_fields = extract_bridge_fields_with_context(value, context);
    if !nested_fields.is_empty() {
        if nested_fields.len() == 1
            && let Some(inner) = nested_fields.get("value")
        {
            return inner.clone();
        }
        return serde_json::Value::Object(nested_fields);
    }
    // Fallback: repr()
    value
        .repr()
        .map(|r| serde_json::Value::String(r.to_string()))
        .unwrap_or_else(|_| serde_json::Value::String("<opaque>".to_string()))
}

fn linear_rgba_to_json(value: &Bound<'_, PyAny>) -> Option<serde_json::Value> {
    value.cast::<PyLinearRgba>().ok()?;
    let fields = LINEAR_RGBA_CHANNELS
        .into_iter()
        .map(|name| {
            let channel = value.getattr(name).ok()?.extract::<f64>().ok()?;
            Some((name.to_string(), float_to_json(channel)))
        })
        .collect::<Option<serde_json::Map<_, _>>>()?;
    Some(serde_json::Value::Object(fields))
}

fn color_to_json(
    value: &Bound<'_, PyAny>,
    context: &mut SerializationContext,
) -> Option<serde_json::Value> {
    value.cast::<PyColor>().ok()?;
    let variant = value.get_type().name().ok()?.to_string_lossy().into_owned();
    if variant == "Color" {
        return None;
    }
    let payload = value.getattr("value").ok()?;
    Some(enum_to_json(
        variant,
        EnumPayload::Single(py_value_to_json_inner(&payload, context)),
    ))
}

/// JSON object key for a Python mapping key. `str` passes through, `int`,
/// `bool` and `float` use their Python `str()`, anything else has no key form.
fn json_object_key(key: &Bound<'_, PyAny>) -> Option<String> {
    if let Ok(s) = key.extract::<String>() {
        return Some(s);
    }
    if key.is_instance_of::<PyInt>() || key.is_instance_of::<PyFloat>() {
        return key.str().ok().map(|s| s.to_string());
    }
    None
}

/// Render an `Entity` as the id the rest of the control API accepts, rather
/// than letting it fall through to an unusable repr.
fn entity_to_json(value: &Bound<'_, PyAny>) -> Option<serde_json::Value> {
    if value.get_type().name().ok()?.to_str().ok()? != "Entity" {
        return None;
    }
    let bits = value.call_method0("to_bits").ok()?.extract::<u64>().ok()?;
    Some(serde_json::json!(bits))
}

/// Handles stay opaque: their variant payload cannot name the asset type a write would need.
fn handle_to_json(value: &Bound<'_, PyAny>) -> Option<serde_json::Value> {
    value.cast::<PyHandle>().ok()?;
    let repr = value.repr().ok()?.extract::<String>().ok()?;
    Some(serde_json::json!({ "repr": repr }))
}

fn python_enum_to_json(value: &Bound<'_, PyAny>) -> Option<serde_json::Value> {
    let enum_base = PyModule::import(value.py(), "enum")
        .ok()?
        .getattr("Enum")
        .ok()?;
    if !value.is_instance(&enum_base).ok()? {
        return None;
    }
    let variant = value.getattr("name").ok()?.extract::<String>().ok()?;
    Some(enum_to_json(variant, EnumPayload::Unit))
}

fn vector_to_json(value: &Bound<'_, PyAny>) -> Option<serde_json::Value> {
    let type_name = value.get_type().name().ok()?;
    let fields = math_shape(FLOAT_VECTORS, type_name.to_str().ok()?)?.coordinates;
    let values = fields
        .iter()
        .map(|field| value.getattr(*field).ok()?.extract::<f64>().ok())
        .collect::<Option<Vec<_>>>()?;
    Some(serde_json::Value::Array(
        values.into_iter().map(float_to_json).collect(),
    ))
}

/// Inner helper: extract getset_descriptor fields from a PyO3 object.
/// Returns an empty map if the object has no such fields.
fn extract_bridge_fields_inner(
    bound: &Bound<'_, PyAny>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut context = SerializationContext {
        active: vec![bound.as_ptr() as usize],
    };
    extract_bridge_fields_with_context(bound, &mut context)
}

fn extract_bridge_fields_with_context(
    bound: &Bound<'_, PyAny>,
    context: &mut SerializationContext,
) -> serde_json::Map<String, serde_json::Value> {
    let py_type = bound.get_type();
    let complex_variant = pyo3_complex_enum_variant_name(&py_type);
    if complex_variant.is_none()
        && let Some(variant) = unit_enum_variant_name(bound, &py_type)
    {
        return enum_to_json(variant, EnumPayload::Unit)
            .as_object()
            .unwrap()
            .clone();
    }
    let mut map = serde_json::Map::new();

    if let Ok(dir_list) = py_type.dir() {
        for attr_obj in dir_list.iter() {
            let Ok(name) = attr_obj.extract::<String>() else {
                continue;
            };
            if name.starts_with('_') {
                continue;
            }
            if !py_type
                .as_any()
                .getattr(name.as_str())
                .is_ok_and(|attr| is_getset_descriptor(&attr))
            {
                continue;
            }
            // Get the actual value from the instance and convert recursively
            if let Ok(value) = bound.getattr(name.as_str()) {
                map.insert(name, py_value_to_json_inner(&value, context));
            }
        }
    }

    // Preserve exact variant identity and use the same canonical enum shape as
    // reflected values and mutation responses.
    if let Some(variant) = complex_variant {
        let payload = if map.is_empty() {
            EnumPayload::Unit
        } else if map.len() == 1 && map.contains_key("value") {
            EnumPayload::Single(map.remove("value").unwrap())
        } else {
            EnumPayload::Named(map)
        };
        let variant = canonical_variant_name(&py_type, &variant);
        return enum_to_json(variant, payload).as_object().unwrap().clone();
    }

    // Collection components expose their contents by iteration, not by
    // properties: Bevy's `Children` keeps its `Vec<Entity>` private and Derefs
    // to a slice, so there is no field to read. Serialize the elements instead
    // of falling through to a repr that omits them.
    if map.is_empty()
        && let Ok(items) = bound.try_iter()
    {
        let elements: Vec<serde_json::Value> = items
            .filter_map(|item| item.ok())
            .map(|item| py_value_to_json_inner(&item, context))
            .collect();
        map.insert("items".to_string(), serde_json::Value::Array(elements));
        return map;
    }

    if map.is_empty()
        && let Some(fields) = constructor_accessor_fields(bound, &py_type, context)
    {
        return fields;
    }

    // Fallback for tuple-struct enum wrappers with no exposed properties.
    if map.is_empty()
        && let Ok(repr) = bound.call_method0("__repr__")
        && let Ok(repr_str) = repr.extract::<String>()
    {
        if let Some(variant) = parse_variant_from_repr(&repr_str) {
            let variant = canonical_variant_name(&py_type, &variant);
            return enum_to_json(variant, EnumPayload::Unit)
                .as_object()
                .unwrap()
                .clone();
        } else {
            map.insert("repr".to_string(), serde_json::Value::String(repr_str));
        }
    }

    map
}

fn constructor_accessor_fields(
    bound: &Bound<'_, PyAny>,
    py_type: &Bound<'_, PyType>,
    context: &mut SerializationContext,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let signature = PyModule::import(bound.py(), "inspect")
        .ok()?
        .call_method1("signature", (py_type,))
        .ok()?;
    let names = signature
        .getattr("parameters")
        .ok()?
        .try_iter()
        .ok()?
        .map(|name| name.ok()?.extract::<String>().ok())
        .collect::<Option<Vec<_>>>()?;
    if names.is_empty() {
        return None;
    }
    let mut map = serde_json::Map::new();
    for name in names {
        let accessor = bound.getattr(name.as_str()).ok()?;
        accessor.cast::<PyCFunction>().ok()?;
        let value = accessor.call0().ok()?;
        map.insert(name, py_value_to_json_inner(&value, context));
    }
    Some(map)
}

fn unit_enum_variant_name(bound: &Bound<'_, PyAny>, owner: &Bound<'_, PyType>) -> Option<String> {
    if !is_enum_type(owner) {
        return None;
    }
    let name = enum_variant_names(owner).into_iter().find(|name| {
        owner.getattr(name.as_str()).is_ok_and(|candidate| {
            candidate.is_instance(owner).unwrap_or(false) && candidate.eq(bound).unwrap_or(false)
        })
    })?;
    Some(canonical_variant_name(owner, &name))
}

fn pyo3_complex_enum_variant_name(py_type: &Bound<'_, PyType>) -> Option<String> {
    let bases = py_type.getattr("__bases__").ok()?;
    let parent = bases.get_item(0).ok()?;
    let parent_type = parent.cast::<PyType>().ok()?;
    if parent_type.name().ok()?.to_string_lossy() == "object" {
        return None;
    }
    for attr in parent_type.dir().ok()?.iter() {
        let name = attr.extract::<String>().ok()?;
        if name.starts_with('_') {
            continue;
        }
        let candidate = parent_type.as_any().getattr(name.as_str()).ok()?;
        if candidate
            .cast::<PyType>()
            .is_ok_and(|candidate| candidate.is(py_type))
        {
            return Some(name);
        }
    }
    None
}

/// Parse the variant name from a Python __repr__ for enum-shaped wrappers.
/// Handles "Class.VARIANT" (uppercase) and "Class(Variant)" (PascalCase) forms.
pub(crate) fn parse_variant_from_repr(repr: &str) -> Option<String> {
    fn is_ident_start(c: char) -> bool {
        c.is_ascii_alphabetic() || c == '_'
    }
    fn is_ident_cont(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }

    let bytes = repr.as_bytes();
    if bytes.is_empty() || !is_ident_start(bytes[0] as char) {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() && is_ident_cont(bytes[i] as char) {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }

    let sep = bytes[i] as char;
    let rest = &repr[i + 1..];

    match sep {
        '.' => {
            // Variant must be a Python identifier ending the string.
            // Accepts SCREAMING_SNAKE (Foo.BAR_BAZ) and mixed case
            // with digits (Msaa.Sample4, Foo.bar_baz).
            if rest.is_empty() {
                return None;
            }
            let rb = rest.as_bytes();
            if !is_ident_start(rb[0] as char) {
                return None;
            }
            for &b in &rb[1..] {
                if !is_ident_cont(b as char) {
                    return None;
                }
            }
            Some(rest.to_string())
        }
        '(' => {
            // Variant must be PascalCase identifier followed by ')'.
            if !rest.ends_with(')') {
                return None;
            }
            let inner = &rest[..rest.len() - 1];
            if inner.is_empty() {
                return None;
            }
            let ib = inner.as_bytes();
            if !is_ident_start(ib[0] as char) {
                return None;
            }
            for &b in &ib[1..] {
                if !is_ident_cont(b as char) {
                    return None;
                }
            }
            Some(inner.to_string())
        }
        _ => None,
    }
}

/// Check if a Python type has any writable getset_descriptor properties (non-underscore).
/// This detects types like Text2d that have settable PyO3 properties even when
/// Bevy's reflection says they are not editable (e.g. TupleStruct types).
pub(crate) fn has_writable_properties(_py: Python<'_>, py_type: &Bound<'_, PyType>) -> bool {
    let Ok(instance) = py_type.call0() else {
        return false;
    };
    let Ok(dir_list) = py_type.dir() else {
        return false;
    };
    for attr_obj in dir_list.iter() {
        let Ok(name) = attr_obj.extract::<String>() else {
            continue;
        };
        if name.starts_with('_') {
            continue;
        }
        if py_type
            .as_any()
            .getattr(name.as_str())
            .is_ok_and(|attr| is_getset_descriptor(&attr))
            && accepts_own_value(&instance, &name)
        {
            return true;
        }
    }
    false
}

/// Extract field values from a bridge (PyO3) component by iterating its `getset_descriptor` properties.
/// Returns a JSON map of field_name → value, recursing into nested structs.
/// Reflected enum values keep the writability probe so their payloads stay
/// replayable; every other value reports every readable field.
pub(crate) fn extract_bridge_fields(
    _py: Python<'_>,
    bound: &Bound<'_, PyAny>,
    reflected_enum: bool,
) -> serde_json::Map<String, serde_json::Value> {
    let mut fields = extract_bridge_fields_inner(bound);
    if bound.is_instance_of::<PyGlobalTransform>() {
        fields.retain(|name, _| matches!(name.as_str(), "translation" | "rotation" | "scale"));
        return fields;
    }
    if !reflected_enum {
        return fields;
    }
    let Ok(owned) = bound.call_method0("__copy__") else {
        if fields.contains_key("variant") {
            fields.retain(|name, _| name == "variant");
        }
        return fields;
    };
    fields.retain(|name, _| name == "variant" || accepts_own_value(&owned, name));
    fields
}

#[cfg(test)]
mod tests {
    use std::{sync::Once, thread};

    use bevy::{color::Color, ecs::world::World, prelude::Transform};
    use pybevy_core::{ValidityGuard, registry::global_registry::all_component_bridges};
    use pyo3::types::{PyAnyMethods, PyDict, PyList, PyTuple};

    // Force linker to include inventory entries needed by these handler tests.
    extern crate pybevy_camera;
    extern crate pybevy_transform;

    use std::f64::consts::PI;

    use pybevy_core::bridge_inventory::collect_all;

    use super::*;

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            Python::initialize();
            collect_all();
        });
    }

    // Test-only value type with a read-only getter and a working `__copy__`,
    // mirroring the shape of the component wrapper classes.
    #[pyclass]
    struct ProbeCopyValue {
        #[pyo3(get)]
        value: f32,
    }

    #[pymethods]
    impl ProbeCopyValue {
        fn __copy__(&self) -> Self {
            Self { value: self.value }
        }
    }

    #[test]
    fn py_value_to_json_primitives() {
        setup();
        Python::attach(|py| {
            // None
            let none = py.None().into_bound(py);
            assert_eq!(py_value_to_json(&none), serde_json::Value::Null);

            // bool - into_pyobject returns Borrowed for bool, use .to_owned()
            let b = true.into_pyobject(py).unwrap().to_owned().into_any();
            assert_eq!(py_value_to_json(&b), serde_json::Value::Bool(true));

            // int
            let i = 42i64.into_pyobject(py).unwrap().into_any();
            assert_eq!(py_value_to_json(&i), serde_json::json!(42));

            // float
            let f = PI.into_pyobject(py).unwrap().into_any();
            let result = py_value_to_json(&f);
            assert!(result.is_number());
            assert!((result.as_f64().unwrap() - PI).abs() < 0.001);

            for (value, spelling) in [
                (f64::NAN, "NaN"),
                (f64::INFINITY, "Infinity"),
                (f64::NEG_INFINITY, "-Infinity"),
            ] {
                let value = value.into_pyobject(py).unwrap().into_any();
                assert_eq!(py_value_to_json(&value), serde_json::json!(spelling));
            }

            // str
            let s = "hello".into_pyobject(py).unwrap().into_any();
            assert_eq!(
                py_value_to_json(&s),
                serde_json::Value::String("hello".into())
            );
        });
    }

    #[test]
    fn py_value_to_json_list() {
        setup();
        Python::attach(|py| {
            let list = PyList::new(py, [1i64, 2, 3]).unwrap();
            let result = py_value_to_json(list.as_any());
            assert!(result.is_array());
            let arr = result.as_array().unwrap();
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], serde_json::json!(1));
            assert_eq!(arr[1], serde_json::json!(2));
            assert_eq!(arr[2], serde_json::json!(3));
        });
    }

    #[test]
    fn serialization_is_bounded_on_a_two_mebibyte_stack() {
        setup();
        thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                Python::attach(|py| {
                    let globals = PyDict::new(py);
                    py.run(
                        ffi::c_str!(
                            "from dataclasses import dataclass\n@dataclass\nclass Payload:\n    child: object\nvalue = 'leaf'\nfor _ in range(512):\n    value = Payload(value)\n"
                        ),
                        Some(&globals),
                        None,
                    )
                    .unwrap();
                    let value = globals.get_item("value").unwrap().unwrap();
                    let result = py_value_to_json(&value);
                    let mut child = &result;
                    for _ in 0..64 {
                        child = child.get("child").unwrap();
                    }
                    assert_eq!(
                        child,
                        &serde_json::json!({
                            "serialization_error": "Python value exceeds the serialization depth limit"
                        })
                    );
                });
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn py_value_to_json_dict_keeps_values() {
        setup();
        Python::attach(|py| {
            let dict = PyDict::new(py);
            dict.set_item("a", 1i64).unwrap();
            dict.set_item("b", 2i64).unwrap();

            let result = py_value_to_json(dict.as_any());

            assert_eq!(result, serde_json::json!({"a": 1, "b": 2}));
        });
    }

    #[test]
    fn py_value_to_json_dict_stringifies_numeric_keys() {
        setup();
        Python::attach(|py| {
            let dict = PyDict::new(py);
            dict.set_item(1i64, "one").unwrap();

            let result = py_value_to_json(dict.as_any());

            assert_eq!(result, serde_json::json!({"1": "one"}));
        });
    }

    #[test]
    fn py_value_to_json_dict_unkeyable_falls_back_to_pairs() {
        setup();
        Python::attach(|py| {
            let dict = PyDict::new(py);
            let key = PyTuple::new(py, [1i64, 2]).unwrap();
            dict.set_item(key, 3i64).unwrap();

            let result = py_value_to_json(dict.as_any());

            assert_eq!(result, serde_json::json!([[[1, 2], 3]]));
        });
    }

    #[test]
    fn py_value_to_json_dataclass_is_structured() {
        setup();
        Python::attach(|py| {
            let globals = PyDict::new(py);
            py.run(
                ffi::c_str!(
                    "import dataclasses\n@dataclasses.dataclass\nclass _Stats:\n    hp: int = 100\n    mp: int = 50\ninst = _Stats()\n"
                ),
                Some(&globals),
                None,
            )
            .unwrap();
            let inst = globals.get_item("inst").unwrap().unwrap();

            let result = py_value_to_json(&inst);

            assert_eq!(result, serde_json::json!({"hp": 100, "mp": 50}));
        });
    }

    #[test]
    fn py_value_to_json_python_enum_member_uses_canonical_unit_shape() {
        setup();
        Python::attach(|py| {
            let globals = PyDict::new(py);
            py.run(
                ffi::c_str!(
                    "from enum import Enum\nclass _Choice(Enum):\n    READY = 3\nvalue = _Choice.READY\n"
                ),
                Some(&globals),
                None,
            )
            .unwrap();
            let value = globals.get_item("value").unwrap().unwrap();

            assert_eq!(
                py_value_to_json(&value),
                serde_json::json!({"variant": "READY"})
            );
        });
    }

    #[test]
    fn py_value_to_json_opaque_object_keeps_repr() {
        setup();
        Python::attach(|py| {
            let globals = PyDict::new(py);
            py.run(
                ffi::c_str!(
                    "class _Opaque:\n    def __repr__(self):\n        return 'opaque value'\ninst = _Opaque()\n"
                ),
                Some(&globals),
                None,
            )
            .unwrap();
            let inst = globals.get_item("inst").unwrap().unwrap();

            let result = py_value_to_json(&inst);

            assert_eq!(result, serde_json::json!({"repr": "opaque value"}));
        });
    }

    #[test]
    fn py_value_to_json_color_uses_canonical_enum_shape() {
        setup();
        Python::attach(|py| {
            let color = PyColor::from_color(Color::srgba(0.2, 1.0, 0.45, 1.0), py).unwrap();

            let result = py_value_to_json(color.bind(py));

            assert_eq!(result["variant"], "Srgba");
            let srgba = result["value"].as_object().unwrap();
            assert!((srgba["red"].as_f64().unwrap() - 0.2).abs() < 1.0e-6);
            assert!((srgba["green"].as_f64().unwrap() - 1.0).abs() < 1.0e-6);
            assert!((srgba["blue"].as_f64().unwrap() - 0.45).abs() < 1.0e-6);
            assert!((srgba["alpha"].as_f64().unwrap() - 1.0).abs() < 1.0e-6);
            assert!(result.get("repr").is_none());
        });
    }

    #[test]
    fn py_value_to_json_linear_rgba_uses_complete_object_form() {
        setup();
        Python::attach(|py| {
            let color = Py::new(py, PyLinearRgba::new(6.5, 3.0, 1.0, 0.75)).unwrap();

            assert_eq!(
                py_value_to_json(color.bind(py)),
                serde_json::json!({
                    "red": 6.5,
                    "green": 3.0,
                    "blue": 1.0,
                    "alpha": 0.75,
                })
            );
        });
    }

    #[test]
    fn py_value_to_json_unrelated_linear_rgba_class_falls_through() {
        setup();
        Python::attach(|py| {
            let globals = PyDict::new(py);
            py.run(
                ffi::c_str!(
                    "class LinearRgba:\n    def __init__(self):\n        self.red = 1.0\n        self.green = 2.0\n        self.blue = 3.0\n        self.alpha = 4.0\n    def __repr__(self):\n        return 'custom LinearRgba'\nvalue = LinearRgba()\n"
                ),
                Some(&globals),
                None,
            )
            .unwrap();
            let value = globals.get_item("value").unwrap().unwrap();

            assert_eq!(
                py_value_to_json(&value),
                serde_json::json!({"repr": "custom LinearRgba"})
            );
        });
    }

    #[test]
    fn py_value_to_json_nested_pyo3_struct() {
        setup();
        // Test that a PyO3 component with getset_descriptor fields produces a dict,
        // not an opaque repr string
        Python::attach(|py| {
            let (_read_window, validity) = ValidityGuard::read();

            let mut world = World::new();
            let entity = world.spawn(Transform::from_xyz(1.0, 2.0, 3.0)).id();

            // Extract the Transform component and check its fields are proper JSON
            for bridge in all_component_bridges() {
                if bridge.name() != "Transform" {
                    continue;
                }
                // SAFETY: `world` is a live World in this test; pointer valid for the call.
                if let Ok(Some(py_obj)) = unsafe {
                    bridge.extract_from_entity_ref(
                        entity,
                        &mut world as *mut World,
                        validity.clone(),
                        py,
                    )
                } {
                    let bound = py_obj.bind(py);
                    let fields = extract_bridge_fields(py, bound, false);

                    // translation should be a nested dict (Vec3 has x, y, z)
                    assert!(
                        fields.contains_key("translation"),
                        "Expected 'translation' field, got: {fields:?}"
                    );
                    let translation = &fields["translation"];
                    // Should be a numeric array, not a repr string like
                    // "<builtins.Vec3 object at 0x...>"
                    let components = translation.as_array().unwrap_or_else(|| {
                        panic!("Expected translation to be an array, got: {translation}")
                    });
                    assert_eq!(components.len(), 3, "Expected x, y, z: {translation}");
                    assert!(
                        components.iter().all(serde_json::Value::is_number),
                        "Expected numeric components, got: {translation}"
                    );
                }
            }
        });
    }

    #[test]
    fn extract_bridge_fields_returns_numeric_values_not_repr_strings() {
        setup();
        // Regression: before Fix 1, all field values were repr() strings.
        // After Fix 1, primitive values should be native JSON types.
        Python::attach(|py| {
            let (_read_window, validity) = ValidityGuard::read();

            let mut world = World::new();
            let entity = world.spawn(Transform::from_xyz(5.0, 10.0, 0.0)).id();

            for bridge in all_component_bridges() {
                if bridge.name() != "Transform" {
                    continue;
                }
                // SAFETY: `world` is a live World in this test; pointer valid for the call.
                if let Ok(Some(py_obj)) = unsafe {
                    bridge.extract_from_entity_ref(
                        entity,
                        &mut world as *mut World,
                        validity.clone(),
                        py,
                    )
                } {
                    let bound = py_obj.bind(py);
                    let fields = extract_bridge_fields(py, bound, false);

                    // scale should be a numeric Vec3 array
                    let scale = &fields["scale"];
                    let components = scale
                        .as_array()
                        .unwrap_or_else(|| panic!("Expected scale to be an array, got: {scale}"));
                    // Default scale is (1, 1, 1)
                    assert_eq!(
                        components
                            .iter()
                            .map(|value| value.as_f64().unwrap())
                            .collect::<Vec<_>>(),
                        vec![1.0, 1.0, 1.0],
                        "Default scale should be (1, 1, 1)"
                    );
                }
            }
        });
    }

    #[test]
    fn plain_value_with_same_type_class_constant_keeps_readable_fields() {
        setup();
        Python::attach(|py| {
            let ty = <ProbeCopyValue as pyo3::PyTypeInfo>::type_object(py);
            let instance = Py::new(py, ProbeCopyValue { value: 7.0 }).unwrap();
            ty.setattr("INSTANCE", instance.clone_ref(py)).unwrap();
            let bound = instance.bind(py).as_any();
            let fields = extract_bridge_fields(py, bound, false);
            assert_eq!(fields.len(), 1);
            assert_eq!(fields["value"].as_f64(), Some(7.0));
        });
    }

    #[test]
    fn has_writable_properties_detects_pyo3_setters() {
        setup();
        // Transform has writable getset_descriptor properties (translation, rotation, scale)
        Python::attach(|py| {
            for bridge in all_component_bridges() {
                if bridge.name() == "Transform" {
                    let py_type = bridge.py_type(py);
                    assert!(
                        has_writable_properties(py, &py_type),
                        "Transform should have writable properties"
                    );
                    return;
                }
            }
            panic!("Transform bridge not found");
        });
    }

    #[test]
    fn has_writable_properties_returns_false_for_no_properties() {
        setup();
        // A plain Python class with no getset_descriptors should return false
        Python::attach(|py| {
            // Create a minimal Python class with no properties
            py.run(ffi::c_str!("class _Empty: pass"), None, None)
                .unwrap();
            let cls = py.eval(ffi::c_str!("_Empty"), None, None).unwrap();
            let py_type = cls.cast::<PyType>().unwrap();
            assert!(
                !has_writable_properties(py, py_type),
                "Empty class should not have writable properties"
            );
        });
    }

    #[test]
    fn parse_variant_from_repr_class_dot_variant() {
        // SCREAMING_SNAKE (Tonemapping, Visibility classvar form).
        assert_eq!(
            parse_variant_from_repr("Tonemapping.NONE"),
            Some("NONE".to_string())
        );
        assert_eq!(
            parse_variant_from_repr("Visibility.HIDDEN"),
            Some("HIDDEN".to_string())
        );
        // PascalCase + digits (PyMsaa repr is "Msaa.Sample4").
        assert_eq!(
            parse_variant_from_repr("Msaa.Sample4"),
            Some("Sample4".to_string())
        );
        // Lowercase identifier with underscore is also a valid Python ident.
        assert_eq!(
            parse_variant_from_repr("Foo.bar_baz"),
            Some("bar_baz".to_string())
        );
    }

    #[test]
    fn parse_variant_from_repr_class_paren_variant() {
        assert_eq!(
            parse_variant_from_repr("Visibility(Inherited)"),
            Some("Inherited".to_string())
        );
        assert_eq!(
            parse_variant_from_repr("Msaa(Sample4)"),
            Some("Sample4".to_string())
        );
    }

    #[test]
    fn parse_variant_from_repr_returns_none_for_unparseable() {
        assert_eq!(parse_variant_from_repr("some random string"), None);
        assert_eq!(parse_variant_from_repr("Foo(123)"), None);
        assert_eq!(parse_variant_from_repr("Foo()"), None);
        // After-dot identifier must not start with a digit.
        assert_eq!(parse_variant_from_repr("Foo.123abc"), None);
    }

    #[test]
    fn extract_bridge_fields_inner_fallback_dot_variant() {
        setup();
        // Synthetic class mimicking PyTonemapping: no getset, dot-uppercase repr.
        Python::attach(|py| {
            let globals = PyDict::new(py);
            py.run(
                ffi::c_str!(
                    "class _FakeTonemapping:\n    def __repr__(self):\n        return 'Tonemapping.NONE'\ninst = _FakeTonemapping()\n"
                ),
                Some(&globals),
                None,
            )
            .unwrap();
            let inst = globals.get_item("inst").unwrap().unwrap();
            let map = extract_bridge_fields_inner(&inst);
            assert_eq!(
                map.get("variant").and_then(|v| v.as_str()),
                Some("NONE"),
                "expected variant=NONE, got: {map:?}"
            );
            assert_eq!(map.len(), 1);
        });
    }

    #[test]
    fn extract_bridge_fields_inner_fallback_paren_variant() {
        setup();
        // Synthetic class mimicking PyVisibility: no getset, paren-PascalCase repr.
        Python::attach(|py| {
            let globals = PyDict::new(py);
            py.run(
                ffi::c_str!(
                    "class _FakeVisibility:\n    def __repr__(self):\n        return 'Visibility(Inherited)'\ninst = _FakeVisibility()\n"
                ),
                Some(&globals),
                None,
            )
            .unwrap();
            let inst = globals.get_item("inst").unwrap().unwrap();
            let map = extract_bridge_fields_inner(&inst);
            assert_eq!(
                map.get("variant").and_then(|v| v.as_str()),
                Some("Inherited"),
                "expected variant=Inherited, got: {map:?}"
            );
            assert_eq!(map.len(), 1);
        });
    }

    #[test]
    fn extract_bridge_fields_inner_fallback_msaa_sample4() {
        setup();
        // Regression: PyMsaa's __repr__ is "Msaa.Sample4" (PascalCase
        // variant with digits after the dot). Before the parser was relaxed
        // to accept any Python identifier after the dot, this fell through to
        // the {"repr": ...} branch instead of yielding {"variant": "Sample4"}.
        Python::attach(|py| {
            let globals = PyDict::new(py);
            py.run(
                ffi::c_str!(
                    "class _FakeMsaa:\n    def __repr__(self):\n        return 'Msaa.Sample4'\ninst = _FakeMsaa()\n"
                ),
                Some(&globals),
                None,
            )
            .unwrap();
            let inst = globals.get_item("inst").unwrap().unwrap();
            let map = extract_bridge_fields_inner(&inst);
            assert_eq!(
                map.get("variant").and_then(|v| v.as_str()),
                Some("Sample4"),
                "expected variant=Sample4, got: {map:?}"
            );
            assert!(
                !map.contains_key("repr"),
                "should not fall back to repr, got: {map:?}"
            );
            assert_eq!(map.len(), 1);
        });
    }

    #[test]
    fn extract_bridge_fields_inner_fallback_unparseable_repr() {
        setup();
        // Unparseable repr falls back to {repr: <full>}.
        Python::attach(|py| {
            let globals = PyDict::new(py);
            py.run(
                ffi::c_str!(
                    "class _FakeOpaque:\n    def __repr__(self):\n        return 'opaque thing'\ninst = _FakeOpaque()\n"
                ),
                Some(&globals),
                None,
            )
            .unwrap();
            let inst = globals.get_item("inst").unwrap().unwrap();
            let map = extract_bridge_fields_inner(&inst);
            assert_eq!(
                map.get("repr").and_then(|v| v.as_str()),
                Some("opaque thing")
            );
            assert!(!map.contains_key("variant"));
        });
    }

    #[test]
    fn extract_bridge_fields_inner_struct_unaffected_by_fallback() {
        setup();
        // Regression: struct-shaped components still return their full fields.
        Python::attach(|py| {
            for bridge in all_component_bridges() {
                if bridge.name() != "Transform" {
                    continue;
                }
                let py_type = bridge.py_type(py);
                let inst = py_type.call0().unwrap();
                let map = extract_bridge_fields_inner(&inst);
                assert!(
                    map.contains_key("translation"),
                    "Transform should expose translation, got: {map:?}"
                );
                assert!(
                    !map.contains_key("variant"),
                    "Transform should not get variant fallback, got: {map:?}"
                );
                assert!(
                    !map.contains_key("repr"),
                    "Transform should not get repr fallback, got: {map:?}"
                );
                return;
            }
            panic!("Transform bridge not found");
        });
    }

    #[test]
    fn complex_enum_variant_name_uses_parent_identity() {
        setup();
        Python::attach(|py| {
            let globals = PyDict::new(py);
            py.run(
                c"class Parent: pass\nclass Variant(Parent): pass\nParent.Exponential = Variant",
                Some(&globals),
                None,
            )
            .unwrap();
            let variant = globals
                .get_item("Variant")
                .unwrap()
                .unwrap()
                .cast_into::<PyType>()
                .unwrap();
            assert_eq!(
                pyo3_complex_enum_variant_name(&variant).as_deref(),
                Some("Exponential")
            );
        });
    }
}
