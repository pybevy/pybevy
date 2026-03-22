pub mod asset;
pub mod custom;
pub mod depth;
pub mod diagnostics;
pub mod execute;
pub mod mutate;
pub mod reflect_mutate;
pub mod reload;
pub mod scene;
pub mod schedule;
pub mod screenshot;
pub mod spatial;
pub mod time_control;
pub mod turnaround;

use bevy::ecs::world::World;

use crate::bridge::{ControlError, ControlOperation};

/// Dispatch an MCP operation to the appropriate handler
pub fn dispatch(
    world: &mut World,
    operation: ControlOperation,
) -> Result<serde_json::Value, ControlError> {
    match operation {
        // Read-only scene
        ControlOperation::ListEntities => scene::list_entities(world),
        ControlOperation::GetEntity { entity } => scene::get_entity(world, entity),
        ControlOperation::ListResources => scene::list_resources(world),
        ControlOperation::ListSystems => scene::list_systems(world),
        ControlOperation::QueryEntities { with, without } => {
            scene::query_entities(world, with, without)
        }
        ControlOperation::GetComponentSchema { name } => scene::get_component_schema(world, name),
        ControlOperation::GetComponent { entity, component } => {
            scene::get_component(world, entity, component)
        }
        ControlOperation::GetPerformance => diagnostics::get_performance(world),
        ControlOperation::DebugRegistry => scene::debug_registry(world),
        ControlOperation::GetReloadStatus => reload::get_reload_status(world),
        ControlOperation::GetLastError => reload::get_last_error(world),

        // Write operations
        ControlOperation::SpawnEntity { components } => mutate::spawn_entity(world, components),
        ControlOperation::DespawnEntity { entity } => mutate::despawn_entity(world, entity),
        ControlOperation::SetComponent {
            entity,
            component,
            fields,
        } => mutate::set_component(world, entity, component, fields),
        ControlOperation::RemoveComponent { entity, component } => {
            mutate::remove_component(world, entity, component)
        }
        ControlOperation::InsertResource {
            resource_type,
            value,
        } => mutate::insert_resource(world, resource_type, value),
        ControlOperation::RemoveResource { resource_type } => {
            mutate::remove_resource(world, resource_type)
        }
        ControlOperation::ExecutePython { code } => execute::execute_python(world, code),

        // Time control
        ControlOperation::PauseTime => time_control::pause_time(world),
        ControlOperation::ResumeTime => time_control::resume_time(world),
        ControlOperation::SetTimeScale { scale } => time_control::set_time_scale(world, scale),
        ControlOperation::GetTimeStatus => time_control::get_time_status(world),
        ControlOperation::SeekTime { seconds, pause } => {
            time_control::seek_time(world, seconds, pause)
        }

        // Asset mutation
        ControlOperation::MutateAsset {
            entity,
            component,
            asset_type,
            fields,
        } => asset::mutate_asset(world, entity, component, asset_type, fields),

        // Bounding box
        ControlOperation::GetBoundingBox { entity } => scene::get_bounding_box(world, entity),

        // Scene summary
        ControlOperation::SceneSummary => scene::scene_summary(world),

        // Spatial queries (synchronous)
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

        // Batch mutations
        ControlOperation::BatchMutate { operations } => mutate::batch_mutate(world, operations),

        // Visual (should have been intercepted by control_poll_system for deferred handling)
        ControlOperation::CaptureScreenshot { .. }
        | ControlOperation::CaptureWithGizmos { .. }
        | ControlOperation::CaptureTimeline { .. } => Err(ControlError::internal(
            "Screenshot requests should be deferred",
        )),

        // Reload (should have been intercepted by control_poll_system for deferred handling)
        ControlOperation::TriggerReload { .. } => {
            Err(ControlError::internal("Reload requests should be deferred"))
        }

        // Deferred compound operations
        ControlOperation::ReloadAndCapture { .. } => Err(ControlError::internal(
            "ReloadAndCapture requests should be deferred",
        )),
        ControlOperation::CaptureTurnaround { .. } => Err(ControlError::internal(
            "CaptureTurnaround requests should be deferred",
        )),
        ControlOperation::CaptureDepth { .. } => Err(ControlError::internal(
            "CaptureDepth requests should be deferred",
        )),

        // Custom tools
        ControlOperation::CallCustomTool { name, arguments } => {
            custom::call_custom_tool(world, name, arguments)
        }

        // Plugin configs
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

        // Schedule (should have been intercepted by control_poll_system)
        ControlOperation::SubmitSchedule { .. } => Err(ControlError::internal(
            "SubmitSchedule should be intercepted by control_poll_system",
        )),
    }
}
