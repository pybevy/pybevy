use bevy::ecs::world::World;
use pyo3::prelude::*;

use super::{mutate::convert_field_value, scene::resolve_entity};
use crate::bridge::{ControlError, EntityRef};

/// Modify asset properties (material color, roughness, etc.) live without code reload.
pub fn mutate_asset(
    world: &mut World,
    entity_ref: EntityRef,
    component: String,
    asset_type: String,
    fields: serde_json::Value,
) -> Result<serde_json::Value, ControlError> {
    let entity = resolve_entity(world, &entity_ref)?;
    let entity_id = entity.to_bits();

    let field_obj = fields
        .as_object()
        .ok_or_else(|| ControlError::invalid_params("'fields' must be a JSON object"))?;

    let mut updated_fields = Vec::new();
    let mut errors = Vec::new();

    Python::attach(|py| {
        // 1. Find the component bridge for the handle component (e.g. MeshMaterial3d)
        let comp_bridge = pybevy_core::registry::global_registry::all_component_bridges()
            .into_iter()
            .find(|b| b.name() == component.as_str());

        let Some(comp_bridge) = comp_bridge else {
            errors.push(format!("Component '{component}' not in registry"));
            return;
        };

        // 2. Extract the component to get the PyHandle
        let validity_flag = pybevy_core::ValidityFlag::new_read();
        let validity = validity_flag.with_access_mode(pybevy_core::AccessMode::Read);

        let handle_obj = if let Ok(entity_ref) = world.get_entity(entity) {
            comp_bridge
                .extract_from_entity_ref(&entity_ref, validity, py)
                .ok()
                .flatten()
        } else {
            errors.push(format!("Entity {entity_id} not found"));
            validity_flag.set_invalid();
            return;
        };
        validity_flag.set_invalid();

        let Some(handle_obj) = handle_obj else {
            errors.push(format!(
                "Component '{component}' not found on entity {entity_id}"
            ));
            return;
        };

        // 3. Get the handle from the component (call .handle() method)
        let bound = handle_obj.bind(py);
        let py_handle = match bound.call_method0("handle") {
            Ok(h) => h,
            Err(e) => {
                errors.push(format!(
                    "Component '{component}' has no handle() method: {e}"
                ));
                return;
            }
        };

        // 4. Extract the PyHandle and convert to UntypedHandle
        let handle: PyRef<pybevy_core::PyHandle> = match py_handle.extract() {
            Ok(h) => h,
            Err(e) => {
                errors.push(format!("Failed to extract handle: {e}"));
                return;
            }
        };

        let untyped = match handle.to_untyped_handle() {
            Ok(h) => h,
            Err(e) => {
                errors.push(format!("Failed to convert handle: {e}"));
                return;
            }
        };

        // 5. Find the asset bridge by name
        let asset_bridge =
            pybevy_core::registry::global_registry::get_asset_bridge_by_name(&asset_type);

        let Some(asset_bridge) = asset_bridge else {
            errors.push(format!("Asset type '{asset_type}' not in registry"));
            return;
        };

        // 6. Get mutable access to the asset
        let write_flag = pybevy_core::ValidityFlag::new_write();
        let write_validity = write_flag.with_access_mode(pybevy_core::AccessMode::Write);

        match asset_bridge.get_mut(world, &untyped, write_validity, py) {
            Ok(Some(py_asset)) => {
                let asset_bound = py_asset.bind(py);
                for (field_name, field_value) in field_obj {
                    match convert_field_value(py, asset_bound, field_name, field_value) {
                        Ok(py_value) => {
                            if let Err(e) = asset_bound.setattr(field_name.as_str(), py_value) {
                                errors.push(format!("{field_name}: {e}"));
                            } else {
                                updated_fields.push(field_name.clone());
                            }
                        }
                        Err(e) => {
                            errors.push(format!("{field_name}: {e}"));
                        }
                    }
                }
            }
            Ok(None) => {
                errors.push(format!("Asset not found for handle on entity {entity_id}"));
            }
            Err(e) => {
                errors.push(format!("Failed to get mutable asset: {e}"));
            }
        }

        write_flag.set_invalid();
    });

    let mut result = serde_json::json!({
        "entity_id": entity_id,
        "component": component,
        "asset_type": asset_type,
        "updated_fields": updated_fields,
    });

    if !errors.is_empty() {
        result
            .as_object_mut()
            .unwrap()
            .insert("errors".into(), serde_json::json!(errors));
    }

    Ok(result)
}
