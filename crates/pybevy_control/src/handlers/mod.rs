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
        ControlOperation::ExecutePython { code } => runtime.execute_python(world, code),
        ControlOperation::ListEntities => runtime.list_entities(world),
        ControlOperation::GetEntity { entity } => runtime.get_entity(world, entity),
        ControlOperation::ListResources => runtime.list_resources(world),
        ControlOperation::ListSystems => runtime.list_systems(world),
        ControlOperation::QueryEntities { with, without } => {
            runtime.query_entities(world, with, without)
        }
        ControlOperation::GetComponentSchema { name } => runtime.get_component_schema(world, name),
        ControlOperation::GetComponent { entity, component } => {
            runtime.get_component(world, entity, component)
        }
        ControlOperation::DebugRegistry => runtime.debug_registry(world),
        ControlOperation::SceneSummary => runtime.scene_summary(world),
        ControlOperation::GetBoundingBox { entity } => runtime.get_bounding_box(world, entity),
        ControlOperation::SpawnEntity { components } => runtime.spawn_entity(world, components),
        ControlOperation::SetComponent {
            entity,
            component,
            fields,
        } => runtime.set_component(world, entity, component, fields),
        ControlOperation::RemoveComponent { entity, component } => {
            runtime.remove_component(world, entity, component)
        }
        ControlOperation::InsertResource {
            resource_type,
            value,
        } => runtime.insert_resource(world, resource_type, value),
        ControlOperation::RemoveResource { resource_type } => {
            runtime.remove_resource(world, resource_type)
        }
        ControlOperation::BatchMutate { operations } => runtime.batch_mutate(world, operations),
        ControlOperation::MutateAsset {
            entity,
            component,
            asset_type,
            fields,
        } => runtime.mutate_asset(world, entity, component, asset_type, fields),
        ControlOperation::CallCustomTool { name, arguments } => {
            runtime.call_custom_tool(world, name, arguments)
        }

        ControlOperation::DespawnEntity { entity } => pyo3::mutate::despawn_entity(world, entity),
        ControlOperation::GetPerformance => diagnostics::get_performance(world),
        ControlOperation::GetReloadStatus => reload::get_reload_status(world),
        ControlOperation::GetLastError => reload::get_last_error(world),
        ControlOperation::PauseTime => time_control::pause_time(world),
        ControlOperation::ResumeTime => time_control::resume_time(world),
        ControlOperation::SetTimeScale { scale } => time_control::set_time_scale(world, scale),
        ControlOperation::GetTimeStatus => time_control::get_time_status(world),
        ControlOperation::SeekTime { seconds, pause } => {
            time_control::seek_time(world, seconds, pause)
        }
        ControlOperation::QuerySpatial { entity_a, entity_b } => {
            spatial::query_spatial(world, entity_a, entity_b)
        }
        ControlOperation::QuerySpatialNeighborhood {
            entity,
            radius,
            max_results,
        } => spatial::query_spatial_neighborhood(world, entity, radius, max_results),
        ControlOperation::CheckOverlaps {
            entity,
            include_siblings,
            max_float_gap,
            ground_y,
        } => spatial::check_overlaps(world, entity, include_siblings, max_float_gap, ground_y),
        ControlOperation::CheckAllOverlaps {
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

        ControlOperation::CaptureScreenshot { .. }
        | ControlOperation::CaptureWithGizmos { .. }
        | ControlOperation::CaptureTimeline { .. } => Err(ControlError::internal(
            "Screenshot requests should be deferred",
        )),
        ControlOperation::TriggerReload { .. } => {
            Err(ControlError::internal("Reload requests should be deferred"))
        }
        ControlOperation::ReloadAndCapture { .. } => Err(ControlError::internal(
            "ReloadAndCapture requests should be deferred",
        )),
        ControlOperation::CaptureTurnaround { .. } => Err(ControlError::internal(
            "CaptureTurnaround requests should be deferred",
        )),
        ControlOperation::CaptureDepth { .. } => Err(ControlError::internal(
            "CaptureDepth requests should be deferred",
        )),
        ControlOperation::SubmitSchedule { .. } => Err(ControlError::internal(
            "SubmitSchedule should be intercepted by control_poll_system",
        )),
    }
}
