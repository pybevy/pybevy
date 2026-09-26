use std::sync::Arc;

use bevy::{
    asset::UntypedAssetId,
    ecs::{entity::Entity, world::World},
};
use pybevy_core::{ComponentBridge, PyHandle, ValidityFlag};
use pyo3::{prelude::*, types::PyType};

use crate::{
    asset_reference::{
        ASSET_REFERENCE_FIELD, ASSET_REFERENCE_KEY, AssetReference, AssetReferenceSession,
        AssetReferenceValidationError, AssetReferenceValue,
    },
    bridge::{ControlError, EntityRef},
    handlers::entity::resolve_entity,
};

pub(crate) fn contains_reference_field(
    fields: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    fields
        .get(ASSET_REFERENCE_FIELD)
        .and_then(serde_json::Value::as_object)
        .is_some_and(|value| value.contains_key(ASSET_REFERENCE_KEY))
}

pub(crate) fn set_from_reference(
    world: &mut World,
    entity: Entity,
    component: &str,
    fields: &serde_json::Map<String, serde_json::Value>,
    bridge: &Arc<dyn ComponentBridge>,
) -> Result<serde_json::Value, ControlError> {
    let reference = parse_fields(component, fields)?;
    let handle = resolve(world, &reference)?;
    construct_and_insert(world, entity, component, bridge, handle)?;
    component_reference_fields(world, entity, component, bridge).ok_or_else(|| {
        ControlError::internal(format!(
            "Component '{component}' was inserted but its asset reference could not be read back"
        ))
    })
}

pub(crate) fn spawn_from_reference(
    world: &mut World,
    entity: Entity,
    component: &str,
    fields: &serde_json::Map<String, serde_json::Value>,
    bridge: &Arc<dyn ComponentBridge>,
) -> Result<serde_json::Value, ControlError> {
    set_from_reference(world, entity, component, fields, bridge)
}

pub(crate) fn component_reference_fields(
    world: &mut World,
    entity: Entity,
    component: &str,
    bridge: &Arc<dyn ComponentBridge>,
) -> Option<serde_json::Value> {
    let handle = extract_handle(world, entity, component, bridge).ok()?;
    reference_fields_for_handle(world, entity, component, &handle)
}

pub(crate) fn reference_fields_for_value(
    world: &mut World,
    entity: Entity,
    component: &str,
    value: &Bound<'_, PyAny>,
) -> Option<serde_json::Value> {
    let handle = value
        .getattr(ASSET_REFERENCE_FIELD)
        .ok()?
        .extract::<PyHandle>()
        .ok()?;
    reference_fields_for_handle(world, entity, component, &handle)
}

fn reference_fields_for_handle(
    world: &mut World,
    entity: Entity,
    component: &str,
    handle: &PyHandle,
) -> Option<serde_json::Value> {
    if !can_issue_reference(handle) {
        return None;
    }
    validate_live_asset(world, handle).ok()?;
    let asset_type = handle.asset_type_name()?;
    let identity = handle_identity(handle);
    let session = world.get_resource_or_insert_with(AssetReferenceSession::default);
    let reference = session.issue(entity.to_bits(), component, asset_type, &identity);
    Some(serde_json::json!({
        ASSET_REFERENCE_FIELD: AssetReferenceValue { asset_ref: reference },
    }))
}

fn can_issue_reference(handle: &PyHandle) -> bool {
    handle.has_strong_handle()
}

pub(crate) fn type_has_handle_property(py_type: &Bound<'_, PyType>) -> bool {
    py_type
        .as_any()
        .getattr(ASSET_REFERENCE_FIELD)
        .ok()
        .and_then(|attribute| {
            attribute
                .get_type()
                .name()
                .ok()
                .map(|name| name.to_string())
        })
        .is_some_and(|name| name == "getset_descriptor")
}

fn parse_fields(
    component: &str,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<AssetReference, ControlError> {
    if fields.len() != 1 || !fields.contains_key(ASSET_REFERENCE_FIELD) {
        return Err(ControlError::invalid_params(format!(
            "Failed to set '{component}': an asset handle write accepts exactly the 'handle' field"
        )));
    }
    serde_json::from_value::<AssetReferenceValue>(fields[ASSET_REFERENCE_FIELD].clone())
        .map(|value| value.asset_ref)
        .map_err(|error| {
            ControlError::invalid_params(format!(
                "Failed to set '{component}': invalid asset reference: {error}"
            ))
        })
}

fn resolve(world: &mut World, reference: &AssetReference) -> Result<PyHandle, ControlError> {
    let session_matches = world
        .get_resource_or_insert_with(AssetReferenceSession::default)
        .contains(reference);
    if !session_matches {
        return Err(ControlError::invalid_params(
            "Asset reference belongs to another app session or a completed Full reload",
        ));
    }

    let source = resolve_entity(world, &EntityRef::Id(reference.entity)).map_err(|_| {
        ControlError::invalid_params(format!(
            "Asset reference source entity {} is missing or stale",
            reference.entity
        ))
    })?;
    let source_bridge = pybevy_core::registry::global_registry::all_component_bridges()
        .into_iter()
        .find(|bridge| bridge.name() == reference.component)
        .ok_or_else(|| {
            ControlError::invalid_params(format!(
                "Asset reference source component '{}' is unknown",
                reference.component
            ))
        })?;
    let handle = extract_handle(world, source, &reference.component, &source_bridge)?;
    if !handle.has_strong_handle() {
        return Err(ControlError::invalid_params(
            "Asset reference source no longer owns a strong handle",
        ));
    }
    let actual_type = handle.asset_type_name().ok_or_else(|| {
        ControlError::invalid_params("Asset reference source has an unregistered asset type")
    })?;
    if actual_type != reference.asset_type {
        return Err(ControlError::invalid_params(format!(
            "Asset reference names asset type '{}' but its source holds '{}'",
            reference.asset_type, actual_type
        )));
    }
    let identity = handle_identity(&handle);
    let validation = world
        .resource::<AssetReferenceSession>()
        .validate(reference, &identity);
    if validation == Err(AssetReferenceValidationError::StaleOrForged) {
        return Err(ControlError::invalid_params(
            "Asset reference token is forged or stale because its source handle changed",
        ));
    }
    if validation == Err(AssetReferenceValidationError::WrongSession) {
        return Err(ControlError::invalid_params(
            "Asset reference belongs to another app session or a completed Full reload",
        ));
    }
    validate_live_asset(world, &handle)?;
    Ok(handle)
}

fn extract_handle(
    world: &mut World,
    entity: Entity,
    component: &str,
    bridge: &Arc<dyn ComponentBridge>,
) -> Result<PyHandle, ControlError> {
    if world
        .get_entity(entity)
        .is_ok_and(|entity_ref| !bridge.entity_contains(&entity_ref))
    {
        return Err(ControlError::invalid_params(format!(
            "Asset reference source component '{component}' is missing"
        )));
    }
    let validity_flag = ValidityFlag::new_read();
    let validity = validity_flag.with_access_mode(pybevy_core::AccessMode::Read);
    let result = Python::attach(|py| {
        // SAFETY: the live World is borrowed for this synchronous call and the
        // wrapper is invalidated before this function returns.
        let component_value =
            unsafe { bridge.extract_from_entity_ref(entity, world as *mut World, validity, py) }
                .map_err(|error| ControlError::invalid_params(error.to_string()))?
                .ok_or_else(|| {
                    ControlError::invalid_params(format!(
                        "Asset reference source component '{component}' is missing"
                    ))
                })?;
        let handle_value = component_value
            .bind(py)
            .getattr(ASSET_REFERENCE_FIELD)
            .map_err(|_| {
                ControlError::invalid_params(format!(
                    "Component '{component}' does not expose an asset handle"
                ))
            })?;
        let handle = handle_value.extract::<PyHandle>().map_err(|_| {
            ControlError::invalid_params(format!(
                "Component '{component}' does not expose an asset handle"
            ))
        })?;
        Ok(handle)
    });
    validity_flag.set_invalid();
    result
}

fn validate_live_asset(world: &World, handle: &PyHandle) -> Result<(), ControlError> {
    let type_id = handle.asset_type_id().ok_or_else(|| {
        ControlError::invalid_params("Asset reference source has an unregistered asset type")
    })?;
    let bridge = pybevy_core::registry::global_registry::get_asset_bridge_by_type_id(type_id)
        .ok_or_else(|| {
            ControlError::invalid_params("Asset reference source has an unregistered asset type")
        })?;
    let exists = bridge
        .contains(world, handle.untyped_id())
        .map_err(|error| {
            ControlError::invalid_params(format!(
                "Asset reference could not validate its asset: {error}"
            ))
        })?;
    if !exists {
        return Err(ControlError::invalid_params(
            "Asset reference source handle no longer resolves to a live asset",
        ));
    }
    Ok(())
}

fn construct_and_insert(
    world: &mut World,
    entity: Entity,
    component: &str,
    bridge: &Arc<dyn ComponentBridge>,
    handle: PyHandle,
) -> Result<(), ControlError> {
    Python::attach(|py| {
        let py_type = bridge.py_type(py);
        if !type_has_handle_property(&py_type) {
            return Err(ControlError::invalid_params(format!(
                "Component '{component}' is not an asset-handle component"
            )));
        }
        let handle =
            Py::new(py, handle).map_err(|error| ControlError::invalid_params(error.to_string()))?;
        let instance = py_type.call1((handle,)).map_err(|error| {
            ControlError::invalid_params(format!(
                "Asset reference type does not match component '{component}': {error}"
            ))
        })?;
        bridge.insert(world, entity, &instance).map_err(|error| {
            ControlError::invalid_params(format!(
                "Failed to insert component '{component}' from asset reference: {error}"
            ))
        })
    })
}

fn handle_identity(handle: &PyHandle) -> Vec<u8> {
    let mut identity = Vec::new();
    match handle.untyped_id() {
        UntypedAssetId::Index { index, .. } => {
            identity.push(0);
            identity.extend_from_slice(&index.to_bits().to_le_bytes());
        }
        UntypedAssetId::Uuid { uuid, .. } => {
            identity.push(1);
            identity.extend_from_slice(uuid.as_bytes());
        }
    }
    identity.extend_from_slice(
        &handle
            .logical_type_id()
            .map_or(0, |id| id.get())
            .to_le_bytes(),
    );
    identity
}

#[cfg(test)]
mod tests {
    use std::{any::TypeId, ptr};

    use bevy::asset::UntypedHandle;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn non_owning_uuid_handle_cannot_issue_a_reference() {
        let handle = PyHandle::from_untyped(
            UntypedHandle::Uuid {
                uuid: Uuid::from_u128(7),
                type_id: TypeId::of::<()>(),
            },
            ptr::null(),
        );

        assert!(!handle.has_strong_handle());
        assert!(!can_issue_reference(&handle));
    }
}
