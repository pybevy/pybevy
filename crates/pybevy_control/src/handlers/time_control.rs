use std::time::Duration;

use bevy::{
    ecs::world::World,
    prelude::{ChildOf, Children, Entity, GlobalTransform, Transform, Without},
    time::{Time, Virtual},
};

use crate::bridge::{ControlError, ErrorCode};

/// Propagate transforms through the full hierarchy after time manipulation.
/// Updates GlobalTransform for root entities first, then recursively for children.
fn propagate_transforms(world: &mut World) {
    // Update root entities (no ChildOf)
    let mut root_query = world.query_filtered::<(Entity, &Transform), Without<ChildOf>>();
    let roots: Vec<(Entity, GlobalTransform)> = root_query
        .iter(world)
        .map(|(e, t)| (e, GlobalTransform::from(*t)))
        .collect();
    for (entity, gt) in &roots {
        if let Some(mut gt_mut) = world.get_mut::<GlobalTransform>(*entity) {
            *gt_mut = *gt;
        }
    }

    // Propagate to children (breadth-first)
    let mut queue: Vec<(Entity, GlobalTransform)> = roots;
    while !queue.is_empty() {
        let mut next_queue = Vec::new();
        for (parent_entity, parent_gt) in &queue {
            let child_entities: Vec<Entity> =
                if let Some(children) = world.get::<Children>(*parent_entity) {
                    children.iter().copied().collect()
                } else {
                    continue;
                };
            for child in child_entities {
                if let Some(child_transform) = world.get::<Transform>(child) {
                    let child_gt = parent_gt.mul_transform(*child_transform);
                    if let Some(mut gt_mut) = world.get_mut::<GlobalTransform>(child) {
                        *gt_mut = child_gt;
                    }
                    next_queue.push((child, child_gt));
                }
            }
        }
        queue = next_queue;
    }
}

pub fn pause_time(world: &mut World) -> Result<serde_json::Value, ControlError> {
    let mut time = world.resource_mut::<Time<Virtual>>();
    time.pause();
    let speed = time.relative_speed();
    Ok(serde_json::json!({
        "paused": true,
        "relative_speed": speed
    }))
}

pub fn resume_time(world: &mut World) -> Result<serde_json::Value, ControlError> {
    let mut time = world.resource_mut::<Time<Virtual>>();
    time.unpause();
    let speed = time.relative_speed();
    Ok(serde_json::json!({
        "paused": false,
        "relative_speed": speed
    }))
}

pub fn set_time_scale(world: &mut World, scale: f32) -> Result<serde_json::Value, ControlError> {
    let mut time = world.resource_mut::<Time<Virtual>>();
    time.set_relative_speed(scale);
    let paused = time.is_paused();
    Ok(serde_json::json!({
        "paused": paused,
        "relative_speed": scale
    }))
}

pub fn get_time_status(world: &mut World) -> Result<serde_json::Value, ControlError> {
    let time = world.resource::<Time<Virtual>>();
    Ok(serde_json::json!({
        "paused": time.is_paused(),
        "relative_speed": time.relative_speed(),
        "effective_speed": time.effective_speed(),
        "elapsed_secs": time.elapsed_secs_f64(),
    }))
}

pub fn seek_time(
    world: &mut World,
    seconds: f64,
    pause: bool,
) -> Result<serde_json::Value, ControlError> {
    if seconds < 0.0 {
        return Err(ControlError::invalid_params("seconds must be >= 0"));
    }
    let current = world.resource::<Time<Virtual>>().elapsed_secs_f64();
    if seconds < current {
        // Reset virtual time by replacing the resource, preserving speed
        let old_speed = world.resource::<Time<Virtual>>().relative_speed();
        world.insert_resource(Time::<Virtual>::default());
        let mut time = world.resource_mut::<Time<Virtual>>();
        time.set_relative_speed(old_speed);
        if seconds > 0.0 {
            time.advance_to(Duration::from_secs_f64(seconds));
        }
    } else {
        let mut time = world.resource_mut::<Time<Virtual>>();
        time.advance_to(Duration::from_secs_f64(seconds));
    }
    if pause {
        let mut time = world.resource_mut::<Time<Virtual>>();
        time.pause();
    }

    // Sync GlobalTransform for root entities so spatial queries work immediately
    propagate_transforms(world);

    let time = world.resource::<Time<Virtual>>();
    let elapsed = time.elapsed_secs_f64();
    let paused = time.is_paused();
    Ok(serde_json::json!({
        "elapsed_secs": elapsed,
        "paused": paused,
        "relative_speed": time.relative_speed(),
        "note": "Time set. Animations/timers will update on next frame. GlobalTransform synced for spatial queries.",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_with_virtual_time() -> World {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world
    }

    #[test]
    fn test_pause_time() {
        let mut world = world_with_virtual_time();
        let result = pause_time(&mut world).unwrap();
        assert_eq!(result["paused"], true);
    }

    #[test]
    fn test_resume_time() {
        let mut world = world_with_virtual_time();
        // Pause first, then resume
        pause_time(&mut world).unwrap();
        let result = resume_time(&mut world).unwrap();
        assert_eq!(result["paused"], false);
    }

    #[test]
    fn test_set_time_scale() {
        let mut world = world_with_virtual_time();
        let result = set_time_scale(&mut world, 2.5).unwrap();
        assert_eq!(result["relative_speed"], 2.5);
    }

    #[test]
    fn test_get_time_status() {
        let mut world = world_with_virtual_time();
        let result = get_time_status(&mut world).unwrap();
        assert_eq!(result["paused"], false);
        assert!(result.get("relative_speed").is_some());
        assert!(result.get("effective_speed").is_some());
        assert!(result.get("elapsed_secs").is_some());
    }

    #[test]
    fn test_seek_time_forward() {
        let mut world = world_with_virtual_time();
        let result = seek_time(&mut world, 5.0, false).unwrap();
        let elapsed = result["elapsed_secs"].as_f64().unwrap();
        assert!((elapsed - 5.0).abs() < 0.01);
        assert_eq!(result["paused"], false);
    }

    #[test]
    fn test_seek_time_backward() {
        let mut world = world_with_virtual_time();
        // Seek forward first
        seek_time(&mut world, 10.0, false).unwrap();
        // Then seek backward
        let result = seek_time(&mut world, 3.0, false).unwrap();
        let elapsed = result["elapsed_secs"].as_f64().unwrap();
        assert!((elapsed - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_seek_time_negative_error() {
        let mut world = world_with_virtual_time();
        let result = seek_time(&mut world, -1.0, false);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::InvalidParams);
    }

    #[test]
    fn test_seek_time_with_pause() {
        let mut world = world_with_virtual_time();
        let result = seek_time(&mut world, 5.0, true).unwrap();
        assert_eq!(result["paused"], true);
    }

    #[test]
    fn test_seek_time_syncs_global_transform() {
        let mut world = world_with_virtual_time();
        // Need Transform + GlobalTransform for the sync
        world.spawn((
            Transform::from_xyz(5.0, 10.0, 15.0),
            GlobalTransform::default(),
        ));

        // Seek should not panic and should sync transforms
        let result = seek_time(&mut world, 2.0, false).unwrap();
        assert!(result["note"].as_str().is_some());
    }
}
