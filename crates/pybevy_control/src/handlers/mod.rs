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
    bridge::{
        ControlError, ControlOperation, MutateOp, OtherOp, ReloadOp, SceneOp, SpatialOp, TimeOp,
        VisualOp,
    },
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
        ControlOperation::Scene(op) => dispatch_scene(world, op, runtime),
        ControlOperation::Mutate(op) => dispatch_mutate(world, op, runtime),
        ControlOperation::Time(op) => dispatch_time(world, op),
        ControlOperation::Visual(op) => dispatch_visual(world, op),
        ControlOperation::Reload(op) => dispatch_reload(world, op),
        ControlOperation::Spatial(op) => dispatch_spatial(world, op),
        ControlOperation::Other(op) => dispatch_other(world, op, runtime),
    }
}

fn dispatch_scene(
    world: &mut World,
    op: SceneOp,
    runtime: &mut dyn ControlRuntime,
) -> Result<serde_json::Value, ControlError> {
    match op {
        SceneOp::ListEntities => runtime.list_entities(world),
        SceneOp::GetEntity { entity } => runtime.get_entity(world, entity),
        SceneOp::ListResources => runtime.list_resources(world),
        SceneOp::ListSystems => runtime.list_systems(world),
        SceneOp::QueryEntities { with, without } => runtime.query_entities(world, with, without),
        SceneOp::GetComponentSchema { name } => runtime.get_component_schema(world, name),
        SceneOp::GetComponent { entity, component } => {
            runtime.get_component(world, entity, component)
        }
        SceneOp::SceneSummary => runtime.scene_summary(world),
        SceneOp::GetBoundingBox { entity } => runtime.get_bounding_box(world, entity),
        SceneOp::DebugRegistry => runtime.debug_registry(world),
    }
}

fn dispatch_mutate(
    world: &mut World,
    op: MutateOp,
    runtime: &mut dyn ControlRuntime,
) -> Result<serde_json::Value, ControlError> {
    match op {
        MutateOp::SpawnEntity { components } => runtime.spawn_entity(world, components),
        MutateOp::DespawnEntity { entity } => pyo3::mutate::despawn_entity(world, entity),
        MutateOp::SetComponent {
            entity,
            component,
            fields,
        } => runtime.set_component(world, entity, component, fields),
        MutateOp::RemoveComponent { entity, component } => {
            runtime.remove_component(world, entity, component)
        }
        MutateOp::InsertResource {
            resource_type,
            value,
        } => runtime.insert_resource(world, resource_type, value),
        MutateOp::RemoveResource { resource_type } => runtime.remove_resource(world, resource_type),
        MutateOp::BatchMutate { operations } => runtime.batch_mutate(world, operations),
    }
}

fn dispatch_time(world: &mut World, op: TimeOp) -> Result<serde_json::Value, ControlError> {
    match op {
        TimeOp::PauseTime => time_control::pause_time(world),
        TimeOp::ResumeTime => time_control::resume_time(world),
        TimeOp::SetTimeScale { scale } => time_control::set_time_scale(world, scale),
        TimeOp::GetTimeStatus => time_control::get_time_status(world),
        TimeOp::SeekTime { seconds, pause } => time_control::seek_time(world, seconds, pause),
    }
}

fn dispatch_visual(_world: &mut World, op: VisualOp) -> Result<serde_json::Value, ControlError> {
    // Visual ops should have been intercepted by control_poll_system for deferred handling
    match op {
        VisualOp::CaptureScreenshot { .. }
        | VisualOp::CaptureWithGizmos { .. }
        | VisualOp::CaptureTimeline { .. } => Err(ControlError::internal(
            "Screenshot requests should be deferred",
        )),
        VisualOp::CaptureTurnaround { .. } => Err(ControlError::internal(
            "CaptureTurnaround requests should be deferred",
        )),
        VisualOp::CaptureDepth { .. } => Err(ControlError::internal(
            "CaptureDepth requests should be deferred",
        )),
    }
}

fn dispatch_reload(world: &mut World, op: ReloadOp) -> Result<serde_json::Value, ControlError> {
    match op {
        ReloadOp::GetReloadStatus => reload::get_reload_status(world),
        ReloadOp::GetLastError => reload::get_last_error(world),
        ReloadOp::TriggerReload { .. } => {
            Err(ControlError::internal("Reload requests should be deferred"))
        }
        ReloadOp::ReloadAndCapture { .. } => Err(ControlError::internal(
            "ReloadAndCapture requests should be deferred",
        )),
    }
}

fn dispatch_spatial(world: &mut World, op: SpatialOp) -> Result<serde_json::Value, ControlError> {
    match op {
        SpatialOp::QuerySpatial { entity_a, entity_b } => {
            spatial::query_spatial(world, entity_a, entity_b)
        }
        SpatialOp::QuerySpatialNeighborhood {
            entity,
            radius,
            max_results,
        } => spatial::query_spatial_neighborhood(world, entity, radius, max_results),
        SpatialOp::CheckOverlaps {
            entity,
            include_siblings,
            max_float_gap,
            ground_y,
        } => spatial::check_overlaps(world, entity, include_siblings, max_float_gap, ground_y),
        SpatialOp::CheckAllOverlaps {
            min_penetration,
            max_results,
            max_float_gap,
            ground_y,
            include_siblings,
        } => spatial::check_all_overlaps(
            world,
            min_penetration,
            max_results,
            max_float_gap,
            ground_y,
            include_siblings,
        ),
    }
}

fn dispatch_other(
    world: &mut World,
    op: OtherOp,
    runtime: &mut dyn ControlRuntime,
) -> Result<serde_json::Value, ControlError> {
    match op {
        OtherOp::ExecutePython { code } => runtime.execute_python(world, code),
        OtherOp::GetPerformance => diagnostics::get_performance(world),
        OtherOp::MutateAsset {
            entity,
            component,
            asset_type,
            fields,
        } => runtime.mutate_asset(world, entity, component, asset_type, fields),
        OtherOp::GetConfig { key } => {
            let configs = world.get_resource::<pybevy_core::PluginConfigs>();
            match configs.and_then(|c| c.get(&key).cloned()) {
                Some(value) => Ok(value),
                None => Err(ControlError::not_found(format!(
                    "Config key '{key}' not found"
                ))),
            }
        }
        OtherOp::ListConfigs => {
            let configs = world
                .get_resource::<pybevy_core::PluginConfigs>()
                .map(|c| c.all().clone())
                .unwrap_or_default();
            Ok(serde_json::json!(configs))
        }
        OtherOp::SubmitSchedule { .. } => Err(ControlError::internal(
            "SubmitSchedule should be intercepted by control_poll_system",
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
    use crate::bridge::ErrorCode;
    use crate::{bridge::EntityRef, runtime_pyo3::Pyo3ControlRuntime};

    fn runtime() -> Pyo3ControlRuntime {
        Pyo3ControlRuntime
    }

    #[test]
    fn dispatch_list_entities_empty_world() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::Scene(SceneOp::ListEntities),
            &mut runtime(),
        )
        .unwrap();
        assert_eq!(result["entity_count"], 0);
    }

    #[test]
    fn dispatch_list_systems() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::Scene(SceneOp::ListSystems),
            &mut runtime(),
        )
        .unwrap();
        assert!(result["stages"].is_object());
    }

    #[test]
    fn dispatch_list_resources() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::Scene(SceneOp::ListResources),
            &mut runtime(),
        )
        .unwrap();
        assert!(result["resource_count"].is_number());
    }

    #[test]
    fn dispatch_scene_summary() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::Scene(SceneOp::SceneSummary),
            &mut runtime(),
        )
        .unwrap();
        assert_eq!(result["total_entities"], 0);
    }

    #[test]
    fn dispatch_debug_registry() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::Scene(SceneOp::DebugRegistry),
            &mut runtime(),
        )
        .unwrap();
        assert!(result["component_bridge_count"].is_number());
    }

    #[test]
    fn dispatch_get_performance() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::Other(OtherOp::GetPerformance),
            &mut runtime(),
        )
        .unwrap();
        assert!(result["entity_count"].is_number());
    }

    #[test]
    fn dispatch_batch_mutate_empty() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::Mutate(MutateOp::BatchMutate { operations: vec![] }),
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
            ControlOperation::Visual(VisualOp::CaptureScreenshot {
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
            ControlOperation::Visual(VisualOp::CaptureTurnaround {
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
            ControlOperation::Visual(VisualOp::CaptureDepth {
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
            ControlOperation::Other(OtherOp::GetConfig {
                key: "nonexistent".into(),
            }),
            &mut runtime(),
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn dispatch_list_configs_empty() {
        let mut world = World::new();
        let result = dispatch(
            &mut world,
            ControlOperation::Other(OtherOp::ListConfigs),
            &mut runtime(),
        )
        .unwrap();
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
            ControlOperation::Other(OtherOp::GetConfig {
                key: "test_key".into(),
            }),
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
        let result = dispatch(
            &mut world,
            ControlOperation::Other(OtherOp::ListConfigs),
            &mut runtime(),
        )
        .unwrap();
        assert_eq!(result["key1"], "val1");
        assert_eq!(result["key2"], "val2");
    }

    #[test]
    fn dispatch_despawn_entity_success() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("Target")).id();
        let result = dispatch(
            &mut world,
            ControlOperation::Mutate(MutateOp::DespawnEntity {
                entity: EntityRef::Id(entity.to_bits()),
            }),
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
            ControlOperation::Mutate(MutateOp::SpawnEntity {
                components: serde_json::json!({"NonExistentComponent": {"field": 1}}),
            }),
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
            ControlOperation::Mutate(MutateOp::SpawnEntity {
                components: serde_json::json!({}),
            }),
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
            ControlOperation::Mutate(MutateOp::BatchMutate {
                operations: vec![serde_json::json!({
                    "action": "despawn",
                    "entity_id": entity.to_bits()
                })],
            }),
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
            ControlOperation::Mutate(MutateOp::BatchMutate {
                operations: vec![serde_json::json!({
                    "action": "spawn",
                    "components": {"FakeComponent": {}}
                })],
            }),
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
            ControlOperation::Spatial(SpatialOp::CheckAllOverlaps {
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
        let result = dispatch(
            &mut world,
            ControlOperation::Scene(SceneOp::ListEntities),
            &mut runtime(),
        )
        .unwrap();
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
            ControlOperation::Scene(SceneOp::GetComponentSchema {
                name: "NonExistentComponentXYZ".into(),
            }),
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
            ControlOperation::Spatial(SpatialOp::CheckAllOverlaps {
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
