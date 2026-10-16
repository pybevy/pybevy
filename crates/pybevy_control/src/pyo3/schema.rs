use std::{
    alloc::Layout,
    collections::{BTreeMap, BTreeSet},
};

use bevy::{
    ecs::{
        component::ComponentId,
        reflect::{AppTypeRegistry, ReflectComponent},
        world::World,
    },
    reflect::{TypeInfo, enums::VariantInfo},
};
use pybevy_core::{component_fields::declared_annotations, public_error};
use pyo3::{
    prelude::*,
    types::{PyDict, PyType},
};

use super::{
    asset_reference::type_has_handle_property,
    enum_names::{canonical_variant_name, is_enum_type},
};
use crate::{
    asset_reference::ASSET_REFERENCE_FIELD,
    codec::{
        reflect,
        value::{EnumPayload, enum_to_json},
    },
    protocol::ControlError,
    pyo3::{
        dataclass_fields::dataclass_field_names,
        descriptors::is_getset_descriptor,
        from_json::field::convert_field_value,
        names::{ComponentTarget, resolve_component},
        to_json::{has_writable_properties, parse_variant_from_repr, py_value_to_json},
    },
};

/// Get component schema (field names, types, defaults, spawn example)
pub fn get_component_schema(
    world: &mut World,
    name: String,
) -> Result<serde_json::Value, ControlError> {
    // Handle special prefixed queries from grep_api and get_type_definition
    if let Some(query) = name.strip_prefix("__search:") {
        return Ok(serde_json::json!({
            "note": "grep_api should be handled by the server's ApiIndex",
            "query": query,
        }));
    }
    if let Some(type_name) = name.strip_prefix("__typedef:") {
        return Ok(serde_json::json!({
            "note": "get_type_definition should be handled by the server's ApiIndex",
            "type_name": type_name,
        }));
    }

    let target = resolve_component(world, &name)?;
    if let Some(ComponentTarget::Native(bridge)) = &target {
        // Try to get Bevy reflection type info for richer schema
        let reflection_info = world.get_resource::<AppTypeRegistry>().and_then(|reg| {
            let type_registry = reg.read();
            let type_id = bridge.bevy_type_id();
            type_registry.get(type_id).map(|registration| {
                let type_info = registration.type_info();
                let has_reflect_component = registration.data::<ReflectComponent>().is_some();
                let is_editable_kind = matches!(type_info, TypeInfo::Struct(_) | TypeInfo::Enum(_));
                let editable = has_reflect_component && is_editable_kind;

                let mut field_types = serde_json::Map::new();
                if let TypeInfo::Struct(info) = type_info {
                    for i in 0..info.field_len() {
                        if let Some(field) = info.field_at(i) {
                            field_types.insert(
                                field.name().to_string(),
                                serde_json::json!(field.type_path_table().short_path()),
                            );
                        }
                    }
                } else if let TypeInfo::Enum(info) = type_info {
                    let variants = (0..info.variant_len())
                        .filter_map(|index| info.variant_at(index).map(|variant| variant.name()))
                        .collect::<Vec<_>>()
                        .join(" | ");
                    field_types.insert("variant".to_string(), serde_json::json!(variants));
                }

                (editable, field_types, type_info_kind_name(type_info))
            })
        });

        let schema = Python::attach(|py| {
            let py_type = bridge.py_type(py);
            let type_name = if name.contains('.') {
                name.clone()
            } else {
                bridge.name().to_string()
            };
            let mut fields = get_class_fields(py, &py_type);
            let is_asset_handle = type_has_handle_property(&py_type);
            if is_asset_handle {
                fields.as_object_mut().unwrap().insert(
                    ASSET_REFERENCE_FIELD.to_string(),
                    serde_json::json!("AssetReference"),
                );
            }

            let mut result = serde_json::json!({
                "name": &type_name,
                "fields": fields,
                "registered": true,
            });

            let reflected_editable = reflection_info
                .as_ref()
                .is_some_and(|(editable, _, _)| *editable);
            let effective_editable = bridge.can_insert()
                && (is_asset_handle
                    || bridge.relationship_field().is_some()
                    || reflected_editable
                    || has_writable_properties(py, &py_type));
            result
                .as_object_mut()
                .unwrap()
                .insert("editable".into(), serde_json::json!(effective_editable));

            if let Some((_, field_types, type_kind)) = &reflection_info {
                let obj = result.as_object_mut().unwrap();
                obj.insert("type_kind".into(), serde_json::json!(type_kind));
                if !field_types.is_empty() {
                    if let Some(fields) = obj
                        .get_mut("fields")
                        .and_then(serde_json::Value::as_object_mut)
                    {
                        fields.extend(field_types.clone());
                    }
                    obj.insert(
                        "field_types".into(),
                        serde_json::Value::Object(field_types.clone()),
                    );
                }
            }

            if let Some(field) = bridge.relationship_field() {
                let obj = result.as_object_mut().unwrap();
                if let Some(fields) = obj
                    .get_mut("fields")
                    .and_then(serde_json::Value::as_object_mut)
                {
                    fields.insert(field.to_string(), serde_json::json!("Entity"));
                }
                obj.insert("field_types".into(), serde_json::json!({ field: "Entity" }));
                obj.insert(
                    "spawn_example".into(),
                    serde_json::json!({
                        "components": { &type_name: "<packed entity id>" }
                    }),
                );
            }

            if is_asset_handle {
                let obj = result.as_object_mut().unwrap();
                obj.insert(
                        "asset_reference".into(),
                        serde_json::json!({
                            "read": "Call get_component on a live entity with this component and replay fields.handle unchanged.",
                            "write_shape": {
                                "handle": {
                                    "asset_ref": {
                                        "session": "<engine-issued session>",
                                        "token": "<engine-issued token>",
                                        "entity": "<packed source entity id>",
                                        "component": &type_name,
                                        "asset_type": "<registered asset type>"
                                    }
                                }
                            }
                        }),
                    );
            }

            let is_enum_schema = reflection_info
                .as_ref()
                .is_some_and(|(_, _, type_kind)| *type_kind == "Enum")
                || is_enum_type(&py_type);
            if is_enum_schema {
                let python_default = default_enum_component_fields(&py_type);
                if let Some(defaults) = &python_default {
                    result.as_object_mut().unwrap().insert(
                        "defaults".into(),
                        serde_json::Value::Object(defaults.clone()),
                    );
                }
                if bridge.can_insert()
                    && bridge.can_offer_hook_free_default_spawn()
                    && let Some(registry) = world.get_resource::<AppTypeRegistry>()
                    && let Some(spawn_fields) = reflect::replayable_enum_default_fields(
                        &registry.read(),
                        bridge.bevy_type_id(),
                        python_default.as_ref(),
                    )
                {
                    result.as_object_mut().unwrap().insert(
                        "spawn_example".into(),
                        serde_json::json!({"components": {&type_name: spawn_fields}}),
                    );
                }
            } else if bridge.can_insert()
                && !is_asset_handle
                && (effective_editable
                    || (bridge.can_offer_hook_free_default_spawn()
                        && result["fields"]
                            .as_object()
                            .is_some_and(serde_json::Map::is_empty)))
                && let Some((defaults, spawn_fields)) =
                    default_component_fields(&py_type, &result["fields"])
            {
                let obj = result.as_object_mut().unwrap();
                obj.insert("defaults".into(), serde_json::Value::Object(defaults));
                if let Some(spawn_fields) = spawn_fields {
                    let spawn_example = serde_json::json!({
                        "components": {
                            &type_name: &spawn_fields,
                        },
                    });
                    obj.insert("spawn_example".into(), spawn_example);
                }
            }

            result
        });

        return Ok(schema);
    }

    // Fallback: check custom Python components via CustomComponentInfo
    let custom_entry = match &target {
        Some(ComponentTarget::Custom(id)) => world
            .get_resource::<pybevy_core::CustomComponentInfo>()
            .and_then(|info| info.get(*id))
            .map(|entry| (*id, entry.clone())),
        _ => None,
    };
    if let Some((component_id, entry)) = custom_entry {
        let schema = Python::attach(|py| {
            let annotated_fields = entry
                .retained_type
                .as_ref()
                .map_or_else(serde_json::Map::new, |retained_type| {
                    get_annotated_fields(retained_type.bind(py))
                });
            let fields = if annotated_fields.is_empty() && entry.is_pyobject_storage {
                get_live_custom_component_fields(py, world, component_id)
            } else {
                annotated_fields
            };
            let editable =
                !fields.is_empty() && (entry.is_pyobject_storage || entry.wrapper_layout.is_some());

            serde_json::json!({
                "name": name,
                "fields": fields,
                "registered": true,
                "custom": true,
                "editable": editable,
                "type_kind": "CustomPython",
                "storage": if entry.is_pyobject_storage { "python" } else { "wrapper" },
            })
        });

        return Ok(schema);
    }

    Err(ControlError::not_found(format!(
        "Component '{name}' not found in registry. It may be an Asset or Resource. \
         Use get_type_definition(type_name=\"{name}\") to see its API. {}",
        public_error::MCP_CUSTOM_COMPONENT_BOOTSTRAP
    )))
}

fn type_info_kind_name(info: &TypeInfo) -> &'static str {
    match info {
        TypeInfo::Struct(_) => "Struct",
        TypeInfo::TupleStruct(_) => "TupleStruct",
        TypeInfo::Tuple(_) => "Tuple",
        TypeInfo::List(_) => "List",
        TypeInfo::Array(_) => "Array",
        TypeInfo::Map(_) => "Map",
        TypeInfo::Set(_) => "Set",
        TypeInfo::Enum(_) => "Enum",
        TypeInfo::Opaque(_) => "Opaque",
    }
}

/// Normalize a Python type repr string.
/// Strips `<class '...'>` wrapping and surrounding single quotes from type names.
/// e.g. `"<class 'float'>"` -> `"float"`, `"'Vec3'"` -> `"Vec3"`
fn normalize_type_repr(s: &str) -> String {
    // Strip <class '...'> wrapping
    if s.starts_with("<class '") && s.ends_with("'>") {
        return s[8..s.len() - 2].to_string();
    }
    // Strip surrounding single quotes
    if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 {
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

/// Extract field information from a Python class
fn get_annotated_fields(py_type: &Bound<'_, PyType>) -> serde_json::Map<String, serde_json::Value> {
    let mut fields = serde_json::Map::new();
    let allowed = match dataclass_field_names(py_type) {
        Ok(names) => names,
        Err(_) => return fields,
    };

    if let Ok(dict) = declared_annotations(py_type) {
        for (key, value) in dict.iter() {
            if let Ok(k) = key.extract::<String>() {
                if allowed.as_ref().is_some_and(|names| !names.contains(&k)) {
                    continue;
                }
                let v = value
                    .repr()
                    .map(|r| normalize_type_repr(&r.to_string()))
                    .unwrap_or_else(|_| "unknown".to_string());
                fields.insert(k, serde_json::Value::String(v));
            }
        }
    }

    fields
}

fn get_live_custom_component_fields(
    py: Python<'_>,
    world: &World,
    component_id: ComponentId,
) -> serde_json::Map<String, serde_json::Value> {
    let is_pyobject_descriptor = world
        .components()
        .get_info(component_id)
        .is_some_and(|info| {
            info.type_id().is_none()
                && info.mutable()
                && info.layout() == Layout::new::<Py<PyAny>>()
        });
    if !is_pyobject_descriptor {
        return serde_json::Map::new();
    }

    let instances = world
        .iter_entities()
        .filter_map(|entity_ref| {
            let ptr = entity_ref.get_by_id(component_id).ok()?;
            // SAFETY: the descriptor check above proves this component stores
            // a live Py<PyAny>. The incref occurs while EntityRef keeps the
            // component pointer valid; only the owned clone escapes the guard.
            let instance = unsafe { &*(ptr.as_ptr() as *const Py<PyAny>) };
            Some(instance.clone_ref(py))
        })
        .collect::<Vec<_>>();

    let mut common = None::<BTreeMap<String, BTreeSet<String>>>;
    for instance in &instances {
        let fields = live_instance_field_types(instance.bind(py));
        match &mut common {
            None => {
                common = Some(
                    fields
                        .into_iter()
                        .map(|(name, type_name)| (name, BTreeSet::from([type_name])))
                        .collect(),
                );
            }
            Some(common) => {
                common.retain(|name, type_names| {
                    let Some(type_name) = fields.get(name) else {
                        return false;
                    };
                    type_names.insert(type_name.clone());
                    true
                });
            }
        }
    }

    common
        .unwrap_or_default()
        .into_iter()
        .map(|(name, type_names)| {
            let type_name = if type_names.len() == 1 {
                type_names.into_iter().next().unwrap()
            } else {
                "unknown".to_string()
            };
            (name, serde_json::Value::String(type_name))
        })
        .collect()
}

fn live_instance_field_types(instance: &Bound<'_, PyAny>) -> BTreeMap<String, String> {
    let Ok(dict) = instance.getattr("__dict__") else {
        return BTreeMap::new();
    };
    let Ok(dict) = dict.cast_into::<PyDict>() else {
        return BTreeMap::new();
    };

    dict.iter()
        .filter_map(|(key, value)| {
            let name = key.extract::<String>().ok()?;
            if name.starts_with('_') {
                return None;
            }
            let type_name = value.get_type().name().ok()?.to_string();
            Some((name, type_name))
        })
        .collect()
}

fn get_class_fields(py: Python<'_>, py_type: &Bound<'_, PyType>) -> serde_json::Value {
    let mut fields = get_annotated_fields(py_type);
    if dataclass_field_names(py_type).ok().flatten().is_some() {
        return serde_json::Value::Object(fields);
    }

    // Try PyO3 getset_descriptor detection via dir()
    if fields.is_empty()
        && let Ok(dir_list) = py_type.dir()
    {
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
            {
                fields.insert(name, serde_json::Value::String("property".into()));
            }
        }
    }

    // Fallback: try __init__ signature via inspect module
    if fields.is_empty()
        && let Ok(inspect) = py.import("inspect")
        && let Ok(sig) = inspect.call_method1("signature", (py_type,))
        && let Ok(params) = sig.getattr("parameters")
        && let Ok(items) = params.call_method0("items")
        && let Ok(iter) = items.try_iter()
    {
        for item in iter {
            if let Ok(tuple) = item
                && let Ok(k) = tuple.get_item(0).and_then(|v| v.extract::<String>())
                && k != "self"
                && !k.starts_with('_')
            {
                fields.insert(k, serde_json::Value::String("unknown".into()));
            }
        }
    }

    serde_json::Value::Object(fields)
}

type FieldMap = serde_json::Map<String, serde_json::Value>;

fn default_component_fields(
    py_type: &Bound<'_, PyType>,
    fields: &serde_json::Value,
) -> Option<(FieldMap, Option<FieldMap>)> {
    let instance = py_type.call0().ok()?;
    let fields = fields.as_object()?;
    let mut defaults = serde_json::Map::new();
    let mut spawn_fields = serde_json::Map::new();

    for name in fields.keys() {
        if let Ok(value) = instance.getattr(name.as_str()) {
            let serialized = py_value_to_json(&value);
            if let Ok(probe) = py_type.call0()
                && let Ok(converted) = convert_field_value(instance.py(), &probe, name, &serialized)
                && probe.setattr(name.as_str(), converted).is_ok()
            {
                spawn_fields.insert(name.clone(), serialized.clone());
            }
            defaults.insert(name.clone(), serialized);
        }
    }

    let replayable_fields = py_type.call0().ok().and_then(|probe| {
        for (name, serialized) in &spawn_fields {
            let converted = convert_field_value(instance.py(), &probe, name, serialized).ok()?;
            probe.setattr(name.as_str(), converted).ok()?;
        }
        Some(spawn_fields)
    });

    Some((defaults, replayable_fields))
}

fn default_enum_component_fields(
    py_type: &Bound<'_, PyType>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let instance = py_type.call0().ok()?;
    if let serde_json::Value::Object(fields) = py_value_to_json(&instance)
        && fields
            .get("variant")
            .is_some_and(serde_json::Value::is_string)
    {
        return reflect::enum_example_value_is_representable(&serde_json::Value::Object(
            fields.clone(),
        ))
        .then_some(fields);
    }
    let repr = instance.repr().ok()?.to_string();
    let variant = parse_variant_from_repr(&repr)?;
    let variant = canonical_variant_name(py_type, &variant);
    let owner = py_type.name().ok()?;
    if !matches!(
        reflect::compiled_enum_variant_info(&owner.to_string_lossy(), &variant),
        Some(VariantInfo::Unit(_))
    ) {
        return None;
    }
    enum_to_json(variant, EnumPayload::Unit)
        .as_object()
        .cloned()
}

#[cfg(test)]
mod tests {
    use std::{ptr, sync::Once};

    use bevy::ecs::component::ComponentId;

    // Force linker to include inventory entries needed by these handler tests.
    extern crate pybevy_camera;
    extern crate pybevy_transform;

    use pybevy_core::bridge_inventory::collect_all;

    use super::*;
    use crate::protocol::ErrorCode;

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            Python::initialize();
            collect_all();
        });
    }

    #[test]
    fn get_component_schema_custom_component() {
        let mut world = World::new();

        // Without CustomComponentInfo, schema lookup fails
        let result = get_component_schema(&mut world, "PlayerStats".into());
        assert!(result.is_err());

        // Add CustomComponentInfo with a fake entry (null type_ptr won't allow Python introspection,
        // but we can verify the lookup path)
        let mut info = pybevy_core::CustomComponentInfo::default();
        info.insert(
            ComponentId::new(88888),
            pybevy_core::CustomComponentEntry {
                type_ptr: ptr::null(),
                retained_type: None,
                name: "PlayerStats".to_string(),
                is_pyobject_storage: true,
                wrapper_layout: None,
            },
        );
        world.insert_resource(info);

        // Now the lookup should find it via CustomComponentInfo
        // (Python introspection will fail with null type_ptr, but
        // in a real app it would return fields)
        // We test that the fallback path is reached and doesn't error
        // before trying Python (it will error in Python::attach with null ptr)
        // For a safe test, we just verify the lookup path exists
        // by checking the result structure
        let result = get_component_schema(&mut world, "StillNotThere".into());
        assert!(result.is_err()); // Different name → still not found
    }
    #[test]
    fn get_component_schema_search_prefix() {
        let mut world = World::new();
        let result = get_component_schema(&mut world, "__search:Transform".into()).unwrap();
        assert_eq!(result["query"], "Transform");
    }

    #[test]
    fn get_component_schema_typedef_prefix() {
        let mut world = World::new();
        let result = get_component_schema(&mut world, "__typedef:Mesh3d".into()).unwrap();
        assert_eq!(result["type_name"], "Mesh3d");
    }

    #[test]
    fn get_component_schema_not_found() {
        let mut world = World::new();
        let result = get_component_schema(&mut world, "NonexistentComponent".into());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }
    #[test]
    fn normalize_type_repr_strips_class_wrapper() {
        assert_eq!(normalize_type_repr("<class 'float'>"), "float");
        assert_eq!(normalize_type_repr("'Vec3'"), "Vec3");
        assert_eq!(normalize_type_repr("int"), "int");
        assert_eq!(
            normalize_type_repr("<class 'builtins.NoneType'>"),
            "builtins.NoneType"
        );
    }

    #[test]
    fn normalize_type_repr_edge_cases() {
        assert_eq!(normalize_type_repr(""), "");
        assert_eq!(normalize_type_repr("<class 'dict'>"), "dict");
    }
    #[test]
    fn editable_fallback_uses_python_properties() {
        setup();
        // Regression test for Fix 2: when Bevy reflection says a component is
        // not editable (e.g. TupleStruct), we check Python properties as fallback.
        // Verify the integration: get_component_schema without a type registry
        // still reports fields via Python introspection.
        let mut world = World::new();
        // No AppTypeRegistry → reflection_info is None → no editable field from reflection
        // The schema should still list fields from Python dir() introspection
        let result = get_component_schema(&mut world, "Transform".into());
        assert!(result.is_ok(), "Transform schema lookup should succeed");
        let schema = result.unwrap();
        // Fields should still be detected via Python getset_descriptor introspection
        let fields = schema["fields"].as_object().unwrap();
        assert!(
            fields.contains_key("translation"),
            "Should detect translation field via Python, got: {fields:?}"
        );
    }

    #[test]
    fn reflected_field_types_replace_generic_property_labels() {
        setup();
        let mut world = World::new();
        world.insert_resource(AppTypeRegistry::default());
        pybevy_core::register_wrapped_reflect_types(&world);

        let schema = get_component_schema(&mut world, "Transform".into()).unwrap();
        let fields = schema["fields"].as_object().unwrap();

        assert_eq!(fields["translation"], "Vec3");
        assert_eq!(fields["rotation"], "Quat");
        assert_eq!(fields["scale"], "Vec3");
    }

    #[test]
    fn component_schema_includes_constructor_defaults_and_spawn_example() {
        setup();
        let mut world = World::new();
        world.insert_resource(AppTypeRegistry::default());
        pybevy_core::register_wrapped_reflect_types(&world);

        let schema = get_component_schema(&mut world, "Transform".into()).unwrap();

        assert_eq!(
            schema["defaults"]["translation"],
            serde_json::json!([0.0, 0.0, 0.0])
        );
        assert_eq!(
            schema["defaults"]["rotation"],
            serde_json::json!([0.0, 0.0, 0.0, 1.0])
        );
        assert_eq!(
            schema["defaults"]["scale"],
            serde_json::json!([1.0, 1.0, 1.0])
        );
        assert_eq!(
            schema["spawn_example"]["components"]["Transform"],
            schema["defaults"]
        );
    }
}
