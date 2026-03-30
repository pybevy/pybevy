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
        OtherOp::CallCustomTool { name, arguments } => {
            runtime.call_custom_tool(world, name, arguments)
        }
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
