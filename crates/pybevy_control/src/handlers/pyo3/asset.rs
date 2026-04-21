use bevy::ecs::world::World;
use pyo3::prelude::*;

use super::{mutate::convert_field_value, scene::resolve_entity};
use crate::bridge::{ControlError, EntityRef, ErrorCode};

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

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use bevy::ecs::{name::Name, world::World};
    use pyo3::prelude::*;

    use super::*;

    static INIT: Once = Once::new();

    fn setup_python() {
        INIT.call_once(|| {
            Python::initialize();
        });
    }

    #[test]
    fn mutate_asset_entity_not_found() {
        let mut world = World::new();
        let result = mutate_asset(
            &mut world,
            EntityRef::Id(999999),
            "MeshMaterial3d".into(),
            "StandardMaterial".into(),
            serde_json::json!({"base_color": [1.0, 0.0, 0.0, 1.0]}),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn mutate_asset_entity_not_found_by_name() {
        let mut world = World::new();
        let result = mutate_asset(
            &mut world,
            EntityRef::Name("NonExistent".into()),
            "MeshMaterial3d".into(),
            "StandardMaterial".into(),
            serde_json::json!({"base_color": [1.0, 0.0, 0.0, 1.0]}),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn mutate_asset_fields_not_object() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("TestEntity")).id();
        let result = mutate_asset(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "MeshMaterial3d".into(),
            "StandardMaterial".into(),
            serde_json::json!("not an object"),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::InvalidParams);
    }

    #[test]
    fn mutate_asset_fields_array_rejected() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("TestEntity")).id();
        let result = mutate_asset(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "MeshMaterial3d".into(),
            "StandardMaterial".into(),
            serde_json::json!([1, 2, 3]),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::InvalidParams);
    }

    #[test]
    fn mutate_asset_unknown_component_reports_error() {
        setup_python();
        let mut world = World::new();
        let entity = world.spawn(Name::new("TestEntity")).id();
        let result = mutate_asset(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "NonExistentComponent".into(),
            "StandardMaterial".into(),
            serde_json::json!({"base_color": [1.0, 0.0, 0.0, 1.0]}),
        );
        // Returns Ok with errors array (not a hard error)
        let result = result.unwrap();
        assert!(result["errors"].is_array());
        let errors = result["errors"].as_array().unwrap();
        assert!(!errors.is_empty());
        assert!(errors[0].as_str().unwrap().contains("not in registry"));
    }

    #[test]
    fn mutate_asset_response_structure() {
        setup_python();
        let mut world = World::new();
        let entity = world.spawn(Name::new("TestEntity")).id();
        // Use an unknown component to trigger the soft error path
        let result = mutate_asset(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "FakeComponent".into(),
            "FakeMaterial".into(),
            serde_json::json!({"roughness": 0.5}),
        )
        .unwrap();
        // Verify response structure
        assert!(result["entity_id"].is_number());
        assert_eq!(result["component"], "FakeComponent");
        assert_eq!(result["asset_type"], "FakeMaterial");
        assert!(result["updated_fields"].is_array());
        assert_eq!(result["updated_fields"].as_array().unwrap().len(), 0);
    }
}
