use std::{
    any::TypeId,
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use bevy::{
    ecs::{change_detection::DetectChangesMut, component::ComponentId, world::World},
    time::{Fixed, Real, Time, Virtual},
};
use pybevy_core::{
    CustomResourceInfo, ResourceBridge, ValidityGuard,
    custom_resource::insert_dynamic_resource_value, public_error,
    registry::global_registry::all_resource_bridges,
};
use pyo3::{
    exceptions::PyNotImplementedError,
    ffi::PyObject,
    prelude::*,
    types::{PyDict, PyType},
};

use crate::{
    codec::value::json_kind_name,
    protocol::ControlError,
    pyo3::{
        components::validate_written_fields,
        dataclass_fields::dataclass_field_names,
        errors::{
            drop_read_only_echoes, unknown_field_error, unknown_set_fields, writable_field_names,
        },
        from_json::{
            enums::resolve_enum_instance,
            field::{
                convert_annotated_field_value, convert_committed_resource_enum_field_value,
                convert_field_value,
            },
        },
        inspect::{opaque_value_fields, reflected_type_is_enum},
        names::{ResourceTarget, custom_resource_name, qualified_type_name, resolve_resource},
        state_resource,
        to_json::{extract_bridge_fields, python_object_fields_to_json},
    },
};

/// Queue a state transition from a `{"variant": "Member"}` payload.
///
/// `State`/`NextState` are driven by `set`/`reset`, so the value cannot be
/// written with the ordinary setattr loop.
fn apply_state_variant(
    bound: &Bound<'_, PyAny>,
    kind: state_resource::StateResource,
    resource_type: &str,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, ControlError> {
    let requested = fields
        .get(state_resource::VARIANT)
        .unwrap_or(&serde_json::Value::Null);
    let variant = match requested {
        serde_json::Value::Null => None,
        serde_json::Value::String(name) => Some(name.as_str()),
        other => {
            return Err(ControlError::invalid_params(format!(
                "{resource_type}.variant expects a member name, got {other}"
            )));
        }
    };

    state_resource::write_variant(bound, kind, variant).map_err(ControlError::invalid_params)?;

    Ok(resource_mutation_response(resource_type, false, true))
}

fn resource_mutation_response(
    resource_type: &str,
    inserted: bool,
    custom: bool,
) -> serde_json::Value {
    let mut response = serde_json::json!({
        "resource": resource_type,
        "inserted": inserted,
    });
    if custom {
        response["custom"] = serde_json::Value::Bool(true);
    }
    response
}

fn validate_custom_resource_fields(
    resource_type: &str,
    instance: &Bound<'_, PyAny>,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), ControlError> {
    let py_type = instance.get_type();
    let mut declared = match dataclass_field_names(&py_type) {
        Ok(Some(names)) => names,
        Ok(None) => py_type
            .getattr("__annotations__")
            .ok()
            .and_then(|value| value.cast_into::<PyDict>().ok())
            .map(|descriptors| {
                descriptors
                    .keys()
                    .iter()
                    .filter_map(|key| key.extract::<String>().ok())
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default(),
        Err(error) => return Err(ControlError::internal(error.to_string())),
    };
    if state_resource::classify(instance).is_some() {
        declared.insert(state_resource::VARIANT.to_string());
    }
    let unknown = fields
        .keys()
        .filter(|field| !declared.contains(*field))
        .cloned()
        .collect::<Vec<_>>();

    if unknown.is_empty() {
        return Ok(());
    }

    let valid = if declared.is_empty() {
        "This resource declares no editable fields. Define it with @resource above @dataclass and annotated fields to use set_resource.".to_string()
    } else {
        format!(
            "Valid fields: {}.",
            declared.into_iter().collect::<Vec<_>>().join(", ")
        )
    };
    Err(ControlError::invalid_params(format!(
        "Resource '{resource_type}' has unknown fields: {}. {valid}",
        unknown.join(", ")
    )))
}

/// Insert or update a resource
pub fn insert_resource(
    world: &mut World,
    resource_type: String,
    value: serde_json::Value,
) -> Result<serde_json::Value, ControlError> {
    let target = resolve_resource(world, &resource_type)?;
    // Reject scalar/array values up front; field application requires a JSON object.
    match &value {
        serde_json::Value::Object(_) | serde_json::Value::Null => {}
        other => {
            return Err(ControlError::invalid_params(format!(
                "set_resource value for '{resource_type}' must be a JSON object (got {})",
                json_kind_name(other)
            )));
        }
    }

    // Try bridge registry first
    let bridge_result = Python::attach(|py| {
        if let Some(ResourceTarget::Native(bridge)) = &target {
            // Patch semantics: clone the current value into owned storage,
            // then commit through the existing resource's mutable slot only
            // after every conversion and setter passes.
            if bridge.contains_in_world(world) {
                if let Some(obj) = value.as_object()
                    && !obj.is_empty()
                {
                    let py_resource = bridge.clone_owned(world, py).map_err(|e| {
                        if e.is_instance_of::<PyNotImplementedError>(py) {
                            ControlError::invalid_params(public_error::mcp_resource_not_writable(
                                &resource_type,
                            ))
                        } else {
                            ControlError::internal(format!(
                                "Failed to copy existing resource for atomic patch: {e}"
                            ))
                        }
                    })?;
                    let instance = py_resource.bind(py);

                    if obj.contains_key("variant")
                        && let Some(replacement) = resolve_enum_instance(py, instance, obj)
                            .map_err(|error| {
                                ControlError::invalid_params(format!(
                                    "Failed to set {resource_type}: {error}"
                                ))
                            })?
                    {
                        bridge
                            .commit_owned(world, replacement.bind(py))
                            .map_err(|e| {
                                ControlError::internal(format!(
                                    "Failed to commit atomic resource patch: {e}"
                                ))
                            })?;
                        return Ok(Some(resource_mutation_response(
                            &resource_type,
                            false,
                            false,
                        )));
                    }

                    let read = extract_bridge_fields(
                        py,
                        instance,
                        reflected_type_is_enum(world, bridge.bevy_type_id()),
                    );
                    let read = opaque_value_fields(world, bridge.bevy_type_id(), None, &read)
                        .unwrap_or(read);
                    let obj = &drop_read_only_echoes(instance, obj, &read).map_err(|error| {
                        ControlError::invalid_params(format!(
                            "Failed to set {resource_type}: {error}"
                        ))
                    })?;
                    let unknown = unknown_set_fields(instance, obj.keys());
                    if !unknown.is_empty() {
                        return Err(ControlError::invalid_params(unknown_field_error(
                            &resource_type,
                            &unknown,
                            &writable_field_names(instance),
                        )));
                    }

                    let mut converted = Vec::with_capacity(obj.len());
                    for (field_name, field_value) in obj {
                        match convert_field_value(py, instance, field_name, field_value) {
                            Ok(py_value) => converted.push((field_name, py_value)),
                            Err(e) => {
                                return Err(ControlError::invalid_params(format!(
                                    "Failed to convert {resource_type}.{field_name}: {e}"
                                )));
                            }
                        }
                    }
                    for (field_name, py_value) in converted {
                        if let Err(e) = instance.setattr(field_name.as_str(), py_value) {
                            return Err(ControlError::invalid_params(format!(
                                "Failed to set {resource_type}.{field_name}: {e}"
                            )));
                        }
                    }
                    validate_written_fields(instance, obj).map_err(|error| {
                        ControlError::invalid_params(format!(
                            "Failed to set {resource_type}.{error}"
                        ))
                    })?;
                    bridge.commit_owned(world, instance).map_err(|e| {
                        ControlError::internal(format!(
                            "Failed to commit atomic resource patch: {e}"
                        ))
                    })?;
                }
                return Ok(Some(resource_mutation_response(
                    &resource_type,
                    false,
                    false,
                )));
            }

            // Resource doesn't exist yet: create default and apply fields
            let py_type = bridge.py_type(py);
            match py_type.call0() {
                Ok(instance) => {
                    // Apply field values if provided
                    if let Some(obj) = value.as_object() {
                        for (field_name, field_value) in obj {
                            match convert_field_value(py, &instance, field_name, field_value) {
                                Ok(py_value) => {
                                    if let Err(e) = instance.setattr(field_name.as_str(), py_value)
                                    {
                                        return Err(ControlError::invalid_params(format!(
                                            "Failed to set {resource_type}.{field_name}: {e}"
                                        )));
                                    }
                                }
                                Err(e) => {
                                    return Err(ControlError::invalid_params(format!(
                                        "Failed to convert {resource_type}.{field_name}: {e}"
                                    )));
                                }
                            }
                        }
                        validate_written_fields(&instance, obj).map_err(|error| {
                            ControlError::invalid_params(format!(
                                "Failed to set {resource_type}.{error}"
                            ))
                        })?;
                    }
                    if let Err(e) = bridge.insert(world, &instance) {
                        return Err(ControlError::internal(format!(
                            "Failed to insert resource: {e}"
                        )));
                    }
                    return Ok(Some(resource_mutation_response(
                        &resource_type,
                        true,
                        false,
                    )));
                }
                Err(e) => {
                    return Err(ControlError::internal(format!(
                        "Failed to create default resource: {e}"
                    )));
                }
            }
        }
        Ok(None) // Not found in bridges
    })?;

    if let Some(result) = bridge_result {
        return Ok(result);
    }

    // Fallback: check custom Python resources via CustomResourceInfo
    let custom_entry = match &target {
        Some(ResourceTarget::Custom(id)) => world
            .get_resource::<CustomResourceInfo>()
            .and_then(|info| info.get(*id))
            .map(|entry| (*id, entry.type_ptr)),
        _ => None,
    };

    if let Some((comp_id, type_ptr)) = custom_entry {
        Python::attach(|py| {
            let existing = world.get_resource_by_id(comp_id).map(|ptr| {
                // SAFETY: this registered custom descriptor stores a live Py<PyAny>.
                unsafe { ptr.deref::<Py<PyAny>>() }.clone_ref(py)
            });
            if let Some(existing) = existing {
                let bound = existing.bind(py);
                if let Some(obj) = value.as_object()
                    && !obj.is_empty()
                {
                    validate_custom_resource_fields(&resource_type, bound, obj)?;
                    if let Some(kind) = state_resource::classify(bound) {
                        let response = apply_state_variant(bound, kind, &resource_type, obj)?;
                        if let Some(mut stored) = world.get_resource_mut_by_id(comp_id) {
                            stored.set_changed();
                        }
                        return Ok(response);
                    }
                    let patch = py
                        .import("pybevy.mcp._resource_patch")
                        .and_then(|module| module.call_method1("_ResourcePatch", (bound,)))
                        .map_err(|error| {
                            ControlError::invalid_params(format!(
                                "Failed to copy existing resource for atomic patch: {error}"
                            ))
                        })?;
                    let staged = patch
                        .getattr("value")
                        .map_err(|error| ControlError::internal(error.to_string()))?;
                    let mut converted = Vec::with_capacity(obj.len());
                    for (field_name, field_value) in obj {
                        let py_value = convert_committed_resource_enum_field_value(
                            py,
                            &staged,
                            field_name,
                            field_value,
                        )
                        .and_then(|converted| {
                            converted.map_or_else(
                                || {
                                    convert_annotated_field_value(
                                        py,
                                        &staged,
                                        field_name,
                                        field_value,
                                    )
                                },
                                Ok,
                            )
                        })
                        .map_err(|error| {
                            ControlError::invalid_params(format!(
                                "Failed to convert {resource_type}.{field_name}: {error}"
                            ))
                        })?;
                        converted.push((field_name, py_value));
                    }
                    for (field_name, py_value) in converted {
                        staged
                            .setattr(field_name.as_str(), py_value)
                            .map_err(|error| {
                                ControlError::invalid_params(format!(
                                    "Failed to set {resource_type}.{field_name}: {error}"
                                ))
                            })?;
                    }
                    patch.call_method0("commit").map_err(|error| {
                        ControlError::invalid_params(format!(
                            "Failed to copy existing resource for atomic patch: {error}"
                        ))
                    })?;
                    if let Some(mut stored) = world.get_resource_mut_by_id(comp_id) {
                        stored.set_changed();
                    }
                }
                return Ok(resource_mutation_response(&resource_type, false, true));
            }

            // Resource doesn't exist yet: create default and apply fields
            // SAFETY: the registry entry retains this class in `type_object`, so `type_ptr` stays live.
            let py_type = unsafe { pyo3::Bound::from_borrowed_ptr(py, type_ptr as *mut PyObject) };
            let Ok(cls) = py_type.cast::<PyType>() else {
                return Err(ControlError::internal(
                    "Custom resource type pointer is invalid".to_string(),
                ));
            };

            let instance = cls
                .call0()
                .map_err(|e| ControlError::internal(format!("Failed to create resource: {e}")))?;

            if let Some(obj) = value.as_object() {
                validate_custom_resource_fields(&resource_type, &instance, obj)?;
                let mut converted = Vec::with_capacity(obj.len());
                for (field_name, field_value) in obj {
                    let py_value =
                        convert_annotated_field_value(py, &instance, field_name, field_value)
                            .map_err(|error| {
                                ControlError::invalid_params(format!(
                                    "Failed to convert {resource_type}.{field_name}: {error}"
                                ))
                            })?;
                    converted.push((field_name, py_value));
                }
                for (field_name, py_value) in converted {
                    if let Err(e) = instance.setattr(field_name.as_str(), py_value) {
                        return Err(ControlError::invalid_params(format!(
                            "Failed to set {resource_type}.{field_name}: {e}"
                        )));
                    }
                }
            }

            // SAFETY: custom resource metadata refers to a Py<PyAny> descriptor,
            // and control mutations use only the canonical resource path.
            unsafe { insert_dynamic_resource_value(world, comp_id, instance.unbind()) };

            Ok(resource_mutation_response(&resource_type, true, true))
        })
    } else {
        Err(ControlError::not_found(format!(
            "Resource '{resource_type}' not in registry. {}",
            public_error::MCP_CUSTOM_RESOURCE_BOOTSTRAP
        )))
    }
}

/// Remove a resource from the world
pub fn remove_resource(
    world: &mut World,
    resource_type: String,
) -> Result<serde_json::Value, ControlError> {
    let target = resolve_resource(world, &resource_type)?;
    if let Some(ResourceTarget::Native(bridge)) = &target {
        let type_id = bridge.bevy_type_id();
        if !bridge.can_user_remove()
            || [
                TypeId::of::<Time>(),
                TypeId::of::<Time<Real>>(),
                TypeId::of::<Time<Virtual>>(),
                TypeId::of::<Time<Fixed>>(),
            ]
            .contains(&type_id)
        {
            return Err(ControlError::invalid_params(
                public_error::control_resource_remove_forbidden(&resource_type),
            ));
        }
        let was_present = bridge.contains_in_world(world);
        bridge.remove(world);
        return Ok(serde_json::json!({
            "removed": resource_type,
            "was_present": was_present,
        }));
    }

    // Fallback: check custom Python resources via CustomResourceInfo
    let custom_comp_id = match &target {
        Some(ResourceTarget::Custom(id)) => world
            .get_resource::<CustomResourceInfo>()
            .and_then(|info| info.get(*id))
            .map(|_| *id),
        _ => None,
    };

    if let Some(comp_id) = custom_comp_id {
        let was_present = world.remove_resource_by_id(comp_id);
        return Ok(serde_json::json!({
            "removed": resource_type,
            "was_present": was_present,
        }));
    }

    Err(ControlError::not_found(format!(
        "Resource '{resource_type}' not in registry. {}",
        public_error::MCP_CUSTOM_RESOURCE_BOOTSTRAP
    )))
}

/// Extract field values from a custom Python resource (dataclass or plain object).
/// Returns a JSON map of field_name -> JSON-converted value, or None if no fields found.
fn extract_custom_resource_fields(
    py: Python<'_>,
    bound: &Bound<'_, PyAny>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    // State machines hold their value behind accessors rather than fields, so
    // the control plane synthesizes the same `variant` key it uses for enum
    // components. Checked first: neither has dataclass fields nor a __dict__.
    if let Some(kind) = state_resource::classify(bound) {
        let variant = state_resource::read_variant(bound, kind).ok()?;
        let mut map = serde_json::Map::new();
        map.insert(
            state_resource::VARIANT.to_string(),
            variant.map_or(serde_json::Value::Null, serde_json::Value::String),
        );
        return Some(map);
    }

    python_object_fields_to_json(py, bound)
}

/// Registered resources with their addressable names, in listing order. A
/// short name shared by several resources is replaced by its qualified name.
fn named_resources(world: &World) -> Vec<(String, ResourceTarget)> {
    let bridges = all_resource_bridges();
    let custom_entries: Vec<(ComponentId, String)> = world
        .get_resource::<pybevy_core::CustomResourceInfo>()
        .map(|custom_info| {
            custom_info
                .iter()
                .map(|(comp_id, entry)| (comp_id, entry.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    let mut name_counts: HashMap<String, usize> = HashMap::new();
    for bridge in &bridges {
        *name_counts.entry(bridge.name().to_string()).or_default() += 1;
    }
    for (_, name) in &custom_entries {
        *name_counts.entry(name.clone()).or_default() += 1;
    }
    let shared = |name: &str| name_counts.get(name).is_some_and(|count| *count > 1);

    let mut named = Vec::new();
    for bridge in bridges {
        let name = if shared(bridge.name()) {
            Python::attach(|py| qualified_type_name(&bridge.py_type(py)))
                .unwrap_or_else(|| bridge.name().to_string())
        } else {
            bridge.name().to_string()
        };
        named.push((name, ResourceTarget::Native(bridge)));
    }
    for (comp_id, name) in custom_entries {
        let name = if shared(&name) {
            custom_resource_name(world, comp_id).unwrap_or(name)
        } else {
            name
        };
        named.push((name, ResourceTarget::Custom(comp_id)));
    }
    named
}

/// The addressable name of every registered resource, without reading values.
pub(crate) fn resource_names(world: &World) -> Vec<String> {
    named_resources(world)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

/// List all registered resources
pub fn list_resources(world: &mut World) -> Result<serde_json::Value, ControlError> {
    let resources: Vec<serde_json::Value> = named_resources(world)
        .into_iter()
        .map(|(name, target)| match target {
            ResourceTarget::Native(bridge) => {
                let mut entry = bridge_resource_entry(world, &bridge);
                entry["name"] = serde_json::json!(name);
                entry
            }
            ResourceTarget::Custom(comp_id) => custom_resource_entry(world, comp_id, name),
        })
        .collect();

    Ok(serde_json::json!({
        "resource_count": resources.len(),
        "resources": resources,
    }))
}

/// Get one registered resource by its control-plane type name.
pub fn get_resource(
    world: &mut World,
    resource_type: String,
) -> Result<serde_json::Value, ControlError> {
    match resolve_resource(world, &resource_type)? {
        Some(ResourceTarget::Native(bridge)) => return Ok(bridge_resource_entry(world, &bridge)),
        Some(ResourceTarget::Custom(component_id)) => {
            return Ok(custom_resource_entry(world, component_id, resource_type));
        }
        None => {}
    }

    let resource_bridges = all_resource_bridges();
    if let Some(bridge) = native_resource_control_name(&resource_type, &resource_bridges) {
        return Err(ControlError::not_found(
            public_error::mcp_native_resource_control_name(&resource_type, bridge.name()),
        ));
    }

    Err(ControlError::not_found(format!(
        "Resource '{resource_type}' not found in registry. {}",
        public_error::MCP_CUSTOM_RESOURCE_BOOTSTRAP
    )))
}

fn native_resource_control_name<'a>(
    requested: &str,
    resource_bridges: &'a [Arc<dyn ResourceBridge>],
) -> Option<&'a Arc<dyn ResourceBridge>> {
    // Generic input spellings resolve to distinct bridge-backed Python classes.
    let explicit_name = match requested {
        "ButtonInput[KeyCode]" => Some("ButtonInput"),
        "ButtonInput[MouseButton]" => Some("MouseInput"),
        _ => None,
    };
    if let Some(name) = explicit_name {
        return resource_bridges.iter().find(|bridge| bridge.name() == name);
    }

    let normalized_requested = normalized_resource_control_name(requested);
    resource_bridges
        .iter()
        .find(|bridge| normalized_resource_control_name(bridge.name()) == normalized_requested)
}

fn normalized_resource_control_name(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

fn bridge_resource_entry(world: &World, bridge: &Arc<dyn ResourceBridge>) -> serde_json::Value {
    let present = bridge.contains_in_world(world);
    let mut entry = serde_json::json!({
        "name": bridge.name(),
        "present": present,
    });

    if present {
        let reflected_enum = reflected_type_is_enum(world, bridge.bevy_type_id());
        let fields = Python::attach(|py| {
            let (_read_window, validity) = ValidityGuard::read();
            bridge
                .get(world, validity, py)
                .ok()
                .map(|py_obj| extract_bridge_fields(py, py_obj.bind(py), reflected_enum))
        })
        .map(|fields| {
            opaque_value_fields(world, bridge.bevy_type_id(), None, &fields).unwrap_or(fields)
        });
        if let Some(fields) = fields {
            entry["fields"] = serde_json::Value::Object(fields);
        }
    }

    entry
}

fn custom_resource_entry(
    world: &World,
    component_id: ComponentId,
    name: String,
) -> serde_json::Value {
    let value = world.get_resource_by_id(component_id);
    let mut entry = serde_json::json!({
        "name": name,
        "present": value.is_some(),
        "custom": true,
    });

    if let Some(py_obj) = value {
        let fields = Python::attach(|py| {
            // SAFETY: custom resource metadata refers to a Py<PyAny> descriptor.
            let py_obj = unsafe { py_obj.deref::<Py<PyAny>>() };
            extract_custom_resource_fields(py, py_obj.bind(py))
        });
        if let Some(fields) = fields {
            entry["fields"] = serde_json::Value::Object(fields);
        }
    }

    entry
}

#[cfg(test)]
mod tests {
    use std::{alloc::Layout, ptr, sync::Once};

    use bevy::{
        ecs::component::{ComponentCloneBehavior, ComponentDescriptor, ComponentId, StorageType},
        ptr::OwningPtr,
    };
    use pybevy_core::CustomResourceEntry;
    use pyo3::{
        ffi,
        types::{PyDict, PyInt},
    };

    // Force linker to include inventory entries needed by these handler tests.
    extern crate pybevy_camera;
    extern crate pybevy_transform;

    use pybevy_core::bridge_inventory::collect_all;

    use super::*;
    use crate::{protocol::ErrorCode, test_python::initialize_python};

    static INIT: Once = Once::new();

    fn setup_python() {
        INIT.call_once(|| {
            initialize_python();
        });
    }

    unsafe fn drop_test_py_object(ptr: OwningPtr<'_>) {
        // SAFETY: test_resource_descriptor declares the value as Py<PyAny>.
        unsafe { ptr.drop_as::<Py<PyAny>>() };
    }

    fn register_test_resource(world: &mut World, name: &'static str) -> ComponentId {
        // SAFETY: layout, drop function, and inserted test values all use Py<PyAny>.
        let descriptor = unsafe {
            ComponentDescriptor::new_with_layout(
                name,
                StorageType::Table,
                Layout::new::<Py<PyAny>>(),
                Some(drop_test_py_object),
                true,
                false,
                ComponentCloneBehavior::Default,
                None,
            )
        };
        world.register_component_with_descriptor(descriptor)
    }

    #[test]
    fn remove_resource_not_in_registry() {
        let mut world = World::new();
        let result = remove_resource(&mut world, "FakeResource".to_string());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn insert_resource_custom_via_custom_resource_info() {
        setup_python();

        let mut world = World::new();

        // Register a fake custom resource
        let comp_id = register_test_resource(&mut world, "GameScore");

        // Use a real Python type pointer so PyO3 doesn't crash on null
        let type_ptr = Python::attach(|py| {
            let int_type = py.get_type::<PyInt>();
            int_type.as_type_ptr()
        });

        let mut info = CustomResourceInfo::default();
        info.insert(
            comp_id,
            CustomResourceEntry {
                type_ptr,
                type_object: None,
                name: "GameScore".to_string(),
            },
        );
        world.insert_resource(info);

        // set_resource should find it via CustomResourceInfo and construct int().
        let result = insert_resource(&mut world, "GameScore".to_string(), serde_json::json!({}));
        assert!(
            result.is_ok(),
            "Custom resource insert failed: {:?}",
            result
        );
        assert_eq!(result.unwrap()["custom"], true);

        assert!(world.contains_resource_by_id(comp_id));
    }

    #[test]
    fn insert_resource_custom_empty_patch_preserves_the_change_tick() {
        setup_python();

        let mut world = World::new();
        let comp_id = register_test_resource(&mut world, "GameScore");
        let type_ptr = Python::attach(|py| py.get_type::<PyInt>().as_type_ptr());

        let mut info = CustomResourceInfo::default();
        info.insert(
            comp_id,
            CustomResourceEntry {
                type_ptr,
                type_object: None,
                name: "GameScore".to_string(),
            },
        );
        world.insert_resource(info);

        // First call constructs the value; second call takes the patch branch.
        insert_resource(&mut world, "GameScore".to_string(), serde_json::json!({})).unwrap();
        world.clear_trackers();
        let before = world
            .get_resource_change_ticks_by_id(comp_id)
            .expect("resource present")
            .changed;

        insert_resource(&mut world, "GameScore".to_string(), serde_json::json!({})).unwrap();

        let after = world
            .get_resource_change_ticks_by_id(comp_id)
            .expect("resource present")
            .changed;
        assert_eq!(
            before, after,
            "an empty patch must preserve the change tick"
        );
    }

    #[test]
    fn remove_resource_custom_via_py_pyresource() {
        setup_python();

        let mut world = World::new();

        // Register a fake custom resource
        let comp_id = register_test_resource(&mut world, "MyCustomRes");

        // Create CustomResourceInfo with the entry
        let type_ptr = Python::attach(|py| {
            let int_type = py.get_type::<PyInt>();
            int_type.as_type_ptr()
        });

        let mut info = CustomResourceInfo::default();
        info.insert(
            comp_id,
            CustomResourceEntry {
                type_ptr,
                type_object: None,
                name: "MyCustomRes".to_string(),
            },
        );
        world.insert_resource(info);

        let py_obj = Python::attach(|py| 42i64.into_pyobject(py).unwrap().into_any().unbind());
        // SAFETY: register_test_resource uses the matching Py<PyAny> descriptor.
        unsafe { insert_dynamic_resource_value(&mut world, comp_id, py_obj) };

        // Verify resource is present before removal
        assert!(
            world.contains_resource_by_id(comp_id),
            "Resource should exist before removal"
        );

        let result = remove_resource(&mut world, "MyCustomRes".to_string()).unwrap();
        assert_eq!(result["removed"], "MyCustomRes");
        assert_eq!(result["was_present"], true);

        assert!(
            !world.contains_resource_by_id(comp_id),
            "Resource should be removed from its resource entity"
        );

        let repeated = remove_resource(&mut world, "MyCustomRes".to_string()).unwrap();
        assert_eq!(repeated["was_present"], false);
    }

    #[test]
    fn remove_resource_custom_not_found_without_info() {
        let mut world = World::new();
        // No CustomResourceInfo resource and no bridge → should fail
        let result = remove_resource(&mut world, "NonExistentCustomRes".to_string());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn insert_resource_rejects_string_value() {
        let mut world = World::new();
        let result = insert_resource(
            &mut world,
            "DirectionalLightShadowMap".to_string(),
            serde_json::json!("not-an-object"),
        );
        let err = result.expect_err("string value must be rejected");
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert!(
            err.message.contains("must be a JSON object"),
            "expected guidance message, got: {}",
            err.message
        );
        assert!(
            err.message.contains("string"),
            "expected message to name the type 'string', got: {}",
            err.message
        );
    }

    #[test]
    fn insert_resource_rejects_number_value() {
        let mut world = World::new();
        let result = insert_resource(
            &mut world,
            "DirectionalLightShadowMap".to_string(),
            serde_json::json!(42),
        );
        let err = result.expect_err("number value must be rejected");
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert!(err.message.contains("must be a JSON object"));
        assert!(err.message.contains("number"));
    }

    #[test]
    fn insert_resource_rejects_bool_value() {
        let mut world = World::new();
        let result = insert_resource(
            &mut world,
            "DirectionalLightShadowMap".to_string(),
            serde_json::json!(true),
        );
        let err = result.expect_err("bool value must be rejected");
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert!(err.message.contains("must be a JSON object"));
        assert_eq!(
            err.message,
            "set_resource value for 'DirectionalLightShadowMap' must be a JSON object (got boolean)"
        );
    }

    #[test]
    fn insert_resource_rejects_array_value() {
        let mut world = World::new();
        let result = insert_resource(
            &mut world,
            "DirectionalLightShadowMap".to_string(),
            serde_json::json!([1, 2, 3]),
        );
        let err = result.expect_err("array value must be rejected");
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert!(err.message.contains("must be a JSON object"));
        assert!(err.message.contains("array"));
    }

    #[test]
    fn insert_resource_accepts_null_value() {
        // Null is treated as a no-op patch; the not-found path returns an error,
        // but the validation itself must not reject Null.
        let mut world = World::new();
        let result = insert_resource(
            &mut world,
            "NoSuchResource".to_string(),
            serde_json::Value::Null,
        );
        // Either ok or some non-validation error is acceptable; what matters is that
        // we did not get the InvalidParams "must be a JSON object" rejection.
        if let Err(err) = result {
            assert_ne!(
                err.code,
                ErrorCode::InvalidParams,
                "Null must not be rejected by the value-shape validation, got: {}",
                err.message
            );
        }
    }

    static INIT_WITH_BRIDGES: Once = Once::new();

    fn setup() {
        INIT_WITH_BRIDGES.call_once(|| {
            initialize_python();
            collect_all();
        });
    }

    #[test]
    fn list_resources_empty_registry() {
        let mut world = World::new();
        let result = list_resources(&mut world).unwrap();
        // Result depends on what's registered globally, but should not error
        assert!(result["resource_count"].is_number());
    }

    #[test]
    fn list_resources_includes_custom_resources() {
        setup();
        let mut world = World::new();

        let baseline = list_resources(&mut world).unwrap();
        let baseline_count = baseline["resource_count"].as_u64().unwrap();

        // Add a CustomResourceInfo with a fake entry
        let mut info = pybevy_core::CustomResourceInfo::default();
        info.insert(
            ComponentId::new(99999),
            pybevy_core::CustomResourceEntry {
                type_ptr: ptr::null(),
                type_object: None,
                name: "GameScore".to_string(),
            },
        );
        world.insert_resource(info);

        let result = list_resources(&mut world).unwrap();
        let new_count = result["resource_count"].as_u64().unwrap();
        assert_eq!(new_count, baseline_count + 1);

        // Find the custom resource in results
        let resources = result["resources"].as_array().unwrap();
        let custom = resources.iter().find(|r| r["name"] == "GameScore");
        assert!(
            custom.is_some(),
            "Custom resource GameScore not found in list"
        );
        assert_eq!(custom.unwrap()["custom"], true);
    }

    #[test]
    fn resource_names_are_the_listed_names_in_order() {
        setup();
        let mut world = World::new();
        let mut info = pybevy_core::CustomResourceInfo::default();
        info.insert(
            ComponentId::new(99999),
            pybevy_core::CustomResourceEntry {
                type_ptr: ptr::null(),
                type_object: None,
                name: "GameScore".to_string(),
            },
        );
        world.insert_resource(info);

        let listed: Vec<String> = list_resources(&mut world).unwrap()["resources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|resource| resource["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(resource_names(&world), listed);
        assert!(listed.iter().any(|name| name == "GameScore"));
    }

    #[test]
    fn list_resources_includes_field_values() {
        // An empty world should still return a valid structure.
        // Bridge resources that are present should have a "fields" key.
        let mut world = World::new();
        let result = list_resources(&mut world).unwrap();
        assert!(result["resources"].is_array());

        // Every present bridge resource should have a "fields" key
        let resources = result["resources"].as_array().unwrap();
        for res in resources {
            if res["present"].as_bool() == Some(true) && res["custom"].is_null() {
                assert!(
                    res.get("fields").is_some(),
                    "Present bridge resource '{}' should have 'fields' key",
                    res["name"]
                );
            }
        }
    }

    #[test]
    fn extract_custom_resource_fields_str_unquoted() {
        setup();
        // Regression: dataclass `str` fields used to come back as Python repr()
        // (with surrounding single quotes) instead of native JSON strings.
        Python::attach(|py| {
            let globals = PyDict::new(py);
            py.run(
                ffi::c_str!(
                    "from dataclasses import dataclass\n\
                     @dataclass\n\
                     class _Element:\n    \
                         symbol: str = 'C'\n    \
                         label: str = ''\n    \
                         atomic_number: int = 6\n    \
                         tags: list = None\n\
                     inst = _Element(symbol='N', label='N7', atomic_number=7, tags=['a','b'])\n"
                ),
                Some(&globals),
                None,
            )
            .unwrap();
            let inst = globals.get_item("inst").unwrap().unwrap();
            let map = extract_custom_resource_fields(py, &inst).expect("fields");

            assert_eq!(
                map.get("symbol"),
                Some(&serde_json::Value::String("N".to_string())),
                "str field should be JSON string without quotes, got: {map:?}"
            );
            assert_eq!(
                map.get("label"),
                Some(&serde_json::Value::String("N7".to_string())),
                "str field should not be repr()-quoted, got: {map:?}"
            );
            assert_eq!(
                map.get("atomic_number"),
                Some(&serde_json::json!(7)),
                "int field should be JSON number, got: {map:?}"
            );
            assert_eq!(
                map.get("tags"),
                Some(&serde_json::json!(["a", "b"])),
                "list[str] field should be JSON array of bare strings, got: {map:?}"
            );
        });
    }
}
