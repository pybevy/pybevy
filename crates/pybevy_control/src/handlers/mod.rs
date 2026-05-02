pub mod depth;
pub mod diagnostics;
pub mod pyo3;
pub mod reflect_mutate;
pub mod reload;
pub mod schedule;
pub mod screenshot;
pub mod spatial;
pub mod time_control;
pub mod turnaround;

use bevy::ecs::world::World;

use crate::{
    bridge::{ControlError, ControlOperation},
    runtime::ControlRuntime,
};

/// Dispatch an MCP operation to the appropriate handler.
///
/// Runtime-dependent operations are routed through the `ControlRuntime` trait.
/// Runtime-agnostic operations (time control, spatial, diagnostics, etc.) are
/// called directly.
pub fn dispatch(
    world: &mut World,
    operation: ControlOperation,
    runtime: &mut dyn ControlRuntime,
) -> Result<serde_json::Value, ControlError> {
    match operation {
        ControlOperation::ListEntities => runtime.list_entities(world),
        ControlOperation::GetEntity { entity } => runtime.get_entity(world, entity),
        ControlOperation::ListResources => runtime.list_resources(world),
        ControlOperation::ListSystems => runtime.list_systems(world),
        ControlOperation::QueryEntities(p) => runtime.query_entities(world, p),
        ControlOperation::GetComponentSchema { name } => runtime.get_component_schema(world, name),
        ControlOperation::GetComponent(p) => runtime.get_component(world, p),
        ControlOperation::GetSceneSummary => runtime.scene_summary(world),
        ControlOperation::GetBoundingBox { entity } => runtime.get_bounding_box(world, entity),
        ControlOperation::GetRegistry => runtime.debug_registry(world),
        ControlOperation::SpawnEntity { components } => runtime.spawn_entity(world, components),
        ControlOperation::DespawnEntity { entity } => pyo3::mutate::despawn_entity(world, entity),
        ControlOperation::SetComponent(p) => runtime.set_component(world, p),
        ControlOperation::RemoveComponent(p) => runtime.remove_component(world, p),
        ControlOperation::SetResource(p) => runtime.insert_resource(world, p),
        ControlOperation::RemoveResource { resource_type } => {
            runtime.remove_resource(world, resource_type)
        }
        ControlOperation::Batch { operations } => runtime.batch_mutate(world, operations),
        ControlOperation::PauseTime => time_control::pause_time(world),
        ControlOperation::ResumeTime => time_control::resume_time(world),
        ControlOperation::SetTimeScale { scale } => time_control::set_time_scale(world, scale),
        ControlOperation::GetTimeStatus => time_control::get_time_status(world),
        ControlOperation::SeekTime(p) => time_control::seek_time(world, p),
        ControlOperation::CaptureScreenshot(..)
        | ControlOperation::CaptureWithGizmos(..)
        | ControlOperation::CaptureTimeline(..) => Err(ControlError::internal(
            "Screenshot requests should be deferred",
        )),
        ControlOperation::CaptureTurnaround(..) => Err(ControlError::internal(
            "CaptureTurnaround requests should be deferred",
        )),
        ControlOperation::CaptureDepth(..) => Err(ControlError::internal(
            "CaptureDepth requests should be deferred",
        )),
        ControlOperation::Reload(..) => {
            Err(ControlError::internal("Reload requests should be deferred"))
        }
        ControlOperation::ReloadAndCapture(..) => Err(ControlError::internal(
            "ReloadAndCapture requests should be deferred",
        )),
        ControlOperation::GetReloadStatus => reload::get_reload_status(world),
        ControlOperation::GetLastError => reload::get_last_error(world),
        ControlOperation::QuerySpatial(p) => spatial::query_spatial(world, p),
        ControlOperation::QuerySpatialNeighborhood(p) => {
            spatial::query_spatial_neighborhood(world, p)
        }
        ControlOperation::CheckOverlaps(p) => spatial::check_overlaps(world, p),
        ControlOperation::CheckAllOverlaps(p) => spatial::check_all_overlaps(world, p),
        ControlOperation::RunCode { code } => runtime.execute_python(world, code),
        ControlOperation::GetPerformance => diagnostics::get_performance(world),
        ControlOperation::SetAsset(p) => runtime.mutate_asset(world, p),
        ControlOperation::GetConfig { key } => {
            let configs = world.get_resource::<pybevy_core::PluginConfigs>();
            match configs.and_then(|c| c.get(&key).cloned()) {
                Some(value) => Ok(value),
                None => Err(ControlError::not_found(format!(
                    "Config key '{key}' not found"
                ))),
            }
        }
        ControlOperation::ListConfigs => {
            let configs = world
                .get_resource::<pybevy_core::PluginConfigs>()
                .map(|c| c.all().clone())
                .unwrap_or_default();
            Ok(serde_json::json!(configs))
        }
        ControlOperation::ScheduleActions(..) => Err(ControlError::internal(
            "ScheduleActions should be intercepted by control_poll_system",
        )),
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        MinimalPlugins,
        app::App,
        camera::primitives::Aabb,
        ecs::{name::Name, world::World},
        math::Vec3,
        prelude::GlobalTransform,
    };

    use super::*;
    use crate::{
        bridge::{
            CaptureDepthParams, CaptureScreenshotParams, CaptureTurnaroundParams,
            CheckAllOverlapsParams, EntityRef, ErrorCode,
        },
        runtime_pyo3::Pyo3ControlRuntime,
    };

    fn runtime() -> Pyo3ControlRuntime {
        Pyo3ControlRuntime
    }

    #[test]
    fn dispatch_list_entities_empty_world() {
        let mut world = World::new();
        let result = dispatch(&mut world, ControlOperation::ListEntities, &mut runtime()).unwrap();
        assert_eq!(result["entity_count"], 0);
    }

    #[test]
    fn dispatch_list_systems() {
        let mut world = World::new();
        let result = dispatch(&mut world, ControlOperation::ListSystems, &mut runtime()).unwrap();
        assert!(result["stages"].is_object());
    }

    #[test]
    fn dispatch_list_resources() {
        let mut world = World::new();
        let result = dispatch(&mut world, ControlOperation::ListResources, &mut runtime()).unwrap();
        assert!(result["resource_count"].is_number());
    }

    #[test]
    fn dispatch_scene_summary() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::GetSceneSummary,
            &mut runtime(),
        )
        .unwrap();
        assert_eq!(result["total_entities"], 0);
    }

    #[test]
    fn dispatch_debug_registry() {
        let mut world = World::new();
        let result = dispatch(&mut world, ControlOperation::GetRegistry, &mut runtime()).unwrap();
        assert!(result["component_bridge_count"].is_number());
    }

    #[test]
    fn dispatch_get_performance() {
        let mut world = World::new();
        let result =
            dispatch(&mut world, ControlOperation::GetPerformance, &mut runtime()).unwrap();
        assert!(result["entity_count"].is_number());
    }

    #[test]
    fn dispatch_batch_mutate_empty() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::Batch { operations: vec![] },
            &mut runtime(),
        )
        .unwrap();
        assert_eq!(result["total"], 0);
    }

    #[test]
    fn dispatch_screenshot_returns_deferred_error() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::CaptureScreenshot(CaptureScreenshotParams {
                max_width: None,
                delay_frames: 0,
                position: None,
                look_at: None,
                hide_ui: false,
            }),
            &mut runtime(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("deferred"));
    }

    #[test]
    fn dispatch_turnaround_returns_deferred_error() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::CaptureTurnaround(CaptureTurnaroundParams {
                look_at: None,
                distance: None,
                elevation: None,
                view_count: None,
                include_top: None,
                columns: None,
                max_width: None,
                hide_ui: None,
            }),
            &mut runtime(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("deferred"));
    }

    #[test]
    fn dispatch_depth_returns_deferred_error() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::CaptureDepth(CaptureDepthParams {
                position: None,
                look_at: None,
                sample_points: None,
                grid_density: None,
                include_rgb: None,
                delay_frames: None,
                hide_ui: None,
                max_width: None,
            }),
            &mut runtime(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("deferred"));
    }

    #[test]
    fn dispatch_get_config_not_found() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::GetConfig {
                key: "nonexistent".into(),
            },
            &mut runtime(),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn dispatch_list_configs_empty() {
        let mut world = World::new();
        let result = dispatch(&mut world, ControlOperation::ListConfigs, &mut runtime()).unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn dispatch_get_config_found() {
        let mut world = World::new();
        let mut configs = pybevy_core::PluginConfigs::default();
        configs.insert("test_key", serde_json::json!({"value": 42}));
        world.insert_resource(configs);
        let result = dispatch(
            &mut world,
            ControlOperation::GetConfig {
                key: "test_key".into(),
            },
            &mut runtime(),
        )
        .unwrap();
        assert_eq!(result["value"], 42);
    }

    #[test]
    fn dispatch_list_configs_populated() {
        let mut world = World::new();
        let mut configs = pybevy_core::PluginConfigs::default();
        configs.insert("key1", serde_json::json!("val1"));
        configs.insert("key2", serde_json::json!("val2"));
        world.insert_resource(configs);
        let result = dispatch(&mut world, ControlOperation::ListConfigs, &mut runtime()).unwrap();
        assert_eq!(result["key1"], "val1");
        assert_eq!(result["key2"], "val2");
    }

    #[test]
    fn dispatch_despawn_entity_success() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let result = dispatch(
            &mut world,
            ControlOperation::DespawnEntity {
                entity: EntityRef::Id(entity.to_bits()),
            },
            &mut runtime(),
        )
        .unwrap();
        assert_eq!(result["despawned"], true);
    }

    #[test]
    fn dispatch_spawn_unknown_component_fails_fast() {
        let mut world = World::new();
        let initial_count = world.entities().len();
        let result = dispatch(
            &mut world,
            ControlOperation::SpawnEntity {
                components: serde_json::json!({"NonExistentComponent": {"field": 1}}),
            },
            &mut runtime(),
        );
        // Unknown components -> invalid_params error, no stray entity
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::InvalidParams);
        assert_eq!(world.entities().len(), initial_count);
    }

    #[test]
    fn dispatch_spawn_empty_components_succeeds() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::SpawnEntity {
                components: serde_json::json!({}),
            },
            &mut runtime(),
        )
        .unwrap();
        assert!(result["entity_id"].is_number());
        assert!(result.get("errors").is_none());
    }

    #[test]
    fn dispatch_batch_despawn_reports_ok_status() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let result = dispatch(
            &mut world,
            ControlOperation::Batch {
                operations: vec![serde_json::json!({
                    "action": "despawn",
                    "entity": entity.to_bits()
                })],
            },
            &mut runtime(),
        )
        .unwrap();
        assert_eq!(result["succeeded"], 1);
        assert_eq!(result["partial"], 0);
        let results = result["results"].as_array().unwrap();
        assert_eq!(results[0]["status"], "ok");
    }

    #[test]
    fn dispatch_batch_spawn_unknown_reports_error() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::Batch {
                operations: vec![serde_json::json!({
                    "action": "spawn",
                    "components": {"FakeComponent": {}}
                })],
            },
            &mut runtime(),
        )
        .unwrap();
        // spawn with unknown components now returns error (not partial)
        assert_eq!(result["succeeded"], 0);
        let results = result["results"].as_array().unwrap();
        assert_eq!(results[0]["status"], "error");
    }

    #[test]
    fn dispatch_check_all_overlaps_accepts_include_siblings() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::CheckAllOverlaps(CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: None,
                include_siblings: false,
            }),
            &mut runtime(),
        )
        .unwrap();
        assert_eq!(result["total_entities_with_aabb"], 0);
        assert_eq!(result["overlap_count"], 0);
    }

    #[test]
    fn dispatch_list_entities_includes_label() {
        let mut world = World::new();
        world.spawn(Name::new("TestEntity"));
        let result = dispatch(&mut world, ControlOperation::ListEntities, &mut runtime()).unwrap();
        let entities = result["entities"].as_array().unwrap();
        assert!(!entities.is_empty());
        // Every entity should have a "label" field
        assert!(
            entities[0].get("label").is_some(),
            "Entity missing 'label' field"
        );
    }

    #[test]
    fn dispatch_get_component_schema_with_type_registry() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.register_type::<bevy::prelude::Transform>();
        app.update();

        // Use a component name that is definitely NOT in any bridge registry.
        // (Transform may be registered globally by other tests calling setup().)
        let result = dispatch(
            app.world_mut(),
            ControlOperation::GetComponentSchema {
                name: "NonExistentComponentXYZ".into(),
            },
            &mut runtime(),
        );
        // Not in the bridge registry, so this returns not_found.
        // The test verifies the dispatch path doesn't panic with AppTypeRegistry present.
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn dispatch_check_all_overlaps_detects_shallow_sunken() {
        let mut world = World::new();
        // Entity barely sunken: min_y = -0.005 (between old 0.01 and new 0.001 thresholds)
        world.spawn((
            Aabb::from_min_max(Vec3::new(-1.0, -0.005, -1.0), Vec3::new(1.0, 1.0, 1.0)),
            GlobalTransform::default(),
        ));

        let result = dispatch(
            &mut world,
            ControlOperation::CheckAllOverlaps(CheckAllOverlapsParams {
                min_penetration: None,
                max_results: None,
                max_float_gap: 0.1,
                ground_y: Some(0.0),
                include_siblings: false,
            }),
            &mut runtime(),
        )
        .unwrap();
        // With the old 0.01 threshold this would report 0 sunken.
        // With the new 0.001 threshold, penetration of 0.005 is detected.
        assert_eq!(result["sunken_count"], 1);
    }
}
