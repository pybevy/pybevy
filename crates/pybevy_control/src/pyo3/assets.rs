use std::any::TypeId;

use bevy::{asset::UntypedHandle, ecs::world::World};
use pybevy_core::{
    AccessMode, AssetAccessRegistry, AssetBorrowCounter, PyHandle, ValidityFlag, ValidityGuard,
    ensure_asset_access_registry,
    public_error::mcp_entity_not_found,
    registry::global_registry::{all_component_bridges, get_asset_bridge_by_name},
};
use pyo3::prelude::*;

use super::{descriptors::is_field_descriptor, from_json::field::convert_field_value};
use crate::{
    protocol::{ControlError, EntityRef},
    scene::entity::resolve_entity,
};

/// A handle-valued field of a component.
struct ComponentHandle {
    field: String,
    handle: UntypedHandle,
    asset_type: String,
}

/// Every public field of `component` that reads as an asset handle.
fn component_handles(component: &Bound<'_, PyAny>) -> PyResult<Vec<ComponentHandle>> {
    let class = component.get_type();
    let mut handles = Vec::new();
    for name in class.dir()? {
        let field: String = name.extract()?;
        if field.starts_with('_')
            || !class
                .getattr(field.as_str())
                .is_ok_and(|attribute| is_field_descriptor(&attribute))
        {
            continue;
        }
        let Ok(value) = component.getattr(field.as_str()) else {
            continue;
        };
        let Ok(handle) = value.extract::<PyRef<PyHandle>>() else {
            continue;
        };
        let asset_type = handle.asset_type_name().unwrap_or("Unknown").to_owned();
        let handle = handle.to_untyped_handle()?;
        handles.push(ComponentHandle {
            field,
            handle,
            asset_type,
        });
    }
    Ok(handles)
}

/// The one handle of `component` whose asset type is `expected_type_id`.
fn select_asset_handle(
    handles: Vec<ComponentHandle>,
    expected_type_id: TypeId,
    component: &str,
    requested_asset_type: &str,
) -> Result<UntypedHandle, String> {
    if handles.is_empty() {
        return Err(format!("Component '{component}' has no asset handle field"));
    }
    let (mut matching, other): (Vec<_>, Vec<_>) = handles
        .into_iter()
        .partition(|candidate| candidate.handle.type_id() == expected_type_id);
    match matching.len() {
        1 => Ok(matching.remove(0).handle),
        0 => {
            let found = other
                .iter()
                .map(|candidate| format!("'{}'", candidate.asset_type))
                .collect::<Vec<_>>()
                .join(", ");
            let noun = if other.len() == 1 { "type" } else { "types" };
            Err(format!(
                "Asset type '{requested_asset_type}' does not match component '{component}' handle {noun} {found}"
            ))
        }
        _ => {
            let fields = matching
                .iter()
                .map(|candidate| candidate.field.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "Component '{component}' has several '{requested_asset_type}' handles ({fields}); set_asset cannot choose one"
            ))
        }
    }
}

/// Modify asset properties (material color, roughness, etc.) live without code reload.
pub fn mutate_asset(
    world: &mut World,
    entity_ref: EntityRef,
    component: String,
    asset_type: String,
    fields: serde_json::Value,
) -> Result<serde_json::Value, ControlError> {
    ensure_asset_access_registry(world);
    let entity = resolve_entity(world, &entity_ref)?;
    let entity_id = entity.to_bits();

    let field_obj = fields
        .as_object()
        .ok_or_else(|| ControlError::invalid_params("'fields' must be a JSON object"))?;

    let mut updated_fields = Vec::new();
    let mut errors = Vec::new();

    Python::attach(|py| {
        // Find the component bridge for the handle component (e.g. MeshMaterial3d)
        let comp_bridge = all_component_bridges()
            .into_iter()
            .find(|b| b.name() == component.as_str());

        let Some(comp_bridge) = comp_bridge else {
            errors.push(format!("Component '{component}' not in registry"));
            return;
        };

        let Some(asset_bridge) = get_asset_bridge_by_name(&asset_type) else {
            errors.push(format!("Asset type '{asset_type}' not in registry"));
            return;
        };

        // Extract the component to read its handle fields
        let (read_window, validity) = ValidityGuard::read();

        let handle_obj = if world.get_entity(entity).is_ok() {
            // SAFETY: `world` is a live &mut World; the pointer is valid for this call.
            unsafe {
                comp_bridge.extract_from_entity_ref(entity, world as *mut World, validity, py)
            }
            .ok()
            .flatten()
        } else {
            errors.push(mcp_entity_not_found(entity_id));
            return;
        };
        let untyped = handle_obj
            .ok_or_else(|| format!("Component '{component}' not found on entity {entity_id}"))
            .and_then(|handle_obj| {
                component_handles(handle_obj.bind(py))
                    .map_err(|error| format!("Failed to read '{component}' handles: {error}"))
            })
            .and_then(|handles| {
                select_asset_handle(
                    handles,
                    asset_bridge.bevy_type_id(),
                    &component,
                    &asset_type,
                )
            });
        drop(read_window);

        let untyped = match untyped {
            Ok(handle) => handle,
            Err(error) => {
                errors.push(error);
                return;
            }
        };

        // Get mutable access to the asset
        let write_flag = ValidityFlag::new();
        let _write_guard = ValidityGuard::new(write_flag.clone());
        let write_validity = write_flag.with_access_mode(AccessMode::Write);
        let borrow_counter = {
            let registry = world
                .get_resource::<AssetAccessRegistry>()
                .expect("control asset mutation initializes AssetAccessRegistry");
            AssetBorrowCounter::from_scope(registry.new_scope(
                asset_bridge.bevy_type_id(),
                asset_bridge.name(),
                write_flag.clone(),
                "control:mutate_asset",
            ))
        };

        let world_cell = world.as_unsafe_world_cell();
        match asset_bridge.get_mut(world_cell, untyped.id(), write_validity, borrow_counter, py) {
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
    });

    if updated_fields.is_empty() && !errors.is_empty() {
        return Err(ControlError::invalid_params(errors.join("; ")));
    }

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
    use std::{any::TypeId, sync::Once};

    use bevy::{
        asset::Handle,
        ecs::{name::Name, world::World},
        mesh::Mesh,
        pbr::StandardMaterial,
    };

    use super::*;
    use crate::{protocol::ErrorCode, test_python::initialize_python};

    static INIT: Once = Once::new();

    fn setup_python() {
        INIT.call_once(|| {
            initialize_python();
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

    fn handle_field<A: bevy::asset::Asset>(field: &str, asset_type: &str) -> ComponentHandle {
        ComponentHandle {
            field: field.into(),
            handle: Handle::<A>::default().untyped(),
            asset_type: asset_type.into(),
        }
    }

    #[test]
    fn asset_handle_is_selected_by_its_asset_type() {
        let mismatch = select_asset_handle(
            vec![handle_field::<Mesh>("handle", "Mesh")],
            TypeId::of::<StandardMaterial>(),
            "Mesh3d",
            "StandardMaterial",
        )
        .unwrap_err();
        assert_eq!(
            mismatch,
            "Asset type 'StandardMaterial' does not match component 'Mesh3d' handle type 'Mesh'"
        );

        let selected = select_asset_handle(
            vec![
                handle_field::<StandardMaterial>("material", "StandardMaterial"),
                handle_field::<Mesh>("source", "Mesh"),
            ],
            TypeId::of::<Mesh>(),
            "Holder",
            "Mesh",
        )
        .unwrap();
        assert_eq!(selected.type_id(), TypeId::of::<Mesh>());

        let ambiguous = select_asset_handle(
            vec![
                handle_field::<Mesh>("first", "Mesh"),
                handle_field::<Mesh>("second", "Mesh"),
            ],
            TypeId::of::<Mesh>(),
            "Holder",
            "Mesh",
        )
        .unwrap_err();
        assert_eq!(
            ambiguous,
            "Component 'Holder' has several 'Mesh' handles (first, second); set_asset cannot choose one"
        );

        assert_eq!(
            select_asset_handle(Vec::new(), TypeId::of::<Mesh>(), "Name", "Mesh").unwrap_err(),
            "Component 'Name' has no asset handle field"
        );
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
        let error = result.unwrap_err();
        assert!(error.message.contains("not in registry"));
    }

    #[test]
    fn mutate_asset_response_structure() {
        setup_python();
        let mut world = World::new();
        let entity = world.spawn(Name::new("TestEntity")).id();
        let error = mutate_asset(
            &mut world,
            EntityRef::Id(entity.to_bits()),
            "FakeComponent".into(),
            "FakeMaterial".into(),
            serde_json::json!({"roughness": 0.5}),
        )
        .unwrap_err();
        assert!(error.message.contains("not in registry"));
    }
}
