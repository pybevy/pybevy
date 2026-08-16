use std::time::Duration;

use bevy::{
    ecs::world::World,
    prelude::{ChildOf, Children, Entity, GlobalTransform, Transform, Without},
    time::{Time, Virtual},
};
use pybevy_core::try_duration_from_secs_f64;

use crate::bridge::{ControlError, SeekTimeParams};

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
    if time.relative_speed() == 0.0 {
        time.set_relative_speed(1.0);
    }
    let speed = time.relative_speed();
    Ok(serde_json::json!({
        "paused": false,
        "relative_speed": speed
    }))
}

/// Largest accepted time scale. Larger finite values are rejected because a
/// huge relative_speed makes each frame advance virtual time by
/// min(raw_delta, max_delta) * scale, which drives Bevy's fixed-timestep
/// accumulator into a runaway catch-up loop (spiral of death) that stalls the
/// frame loop for seconds and times out the control channel. 1000x is already
/// far beyond any authoring need (a 24s cycle runs in 24ms).
pub const MAX_TIME_SCALE: f32 = 1000.0;

/// Validate a requested time scale before it reaches Bevy's
/// Time<Virtual>::set_relative_speed, which panics on <= 0.0 and non-finite
/// values and spirals the fixed-timestep loop on very large values.
pub fn validate_time_scale(scale: f32) -> Result<(), ControlError> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err(ControlError::invalid_params("scale must be > 0 and finite"));
    }
    if scale > MAX_TIME_SCALE {
        return Err(ControlError::invalid_params(format!(
            "scale must be <= {MAX_TIME_SCALE} (larger values stall the engine's fixed-timestep loop)"
        )));
    }
    Ok(())
}

pub fn set_time_scale(world: &mut World, scale: f32) -> Result<serde_json::Value, ControlError> {
    validate_time_scale(scale)?;
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
    params: SeekTimeParams,
) -> Result<serde_json::Value, ControlError> {
    let SeekTimeParams { seconds, pause } = params;
    if seconds < 0.0 {
        return Err(ControlError::invalid_params("seconds must be >= 0"));
    }
    // `< 0.0` is false for NaN, and Duration tops out near 1.8e19 seconds.
    let target = try_duration_from_secs_f64(seconds)
        .map_err(|error| ControlError::invalid_params(error.message()))?;
    let current = world.resource::<Time<Virtual>>().elapsed_secs_f64();
    if seconds < current {
        // Reset virtual time by replacing the resource, preserving speed
        let old_speed = world.resource::<Time<Virtual>>().relative_speed();
        world.insert_resource(Time::<Virtual>::default());
        let mut time = world.resource_mut::<Time<Virtual>>();
        time.set_relative_speed(old_speed);
        if seconds > 0.0 {
            time.advance_to(target);
        }
    } else {
        let mut time = world.resource_mut::<Time<Virtual>>();
        time.advance_to(target);
    }
    {
        // advance_to leaves the whole jump as this frame's delta, and the
        // fixed main loop consumes Time<Virtual>::delta directly: a large
        // seek would replay that many FixedMain catch-up iterations in one
        // frame. Nothing else reads this delta (the generic clock was synced
        // earlier in the frame), so zero it.
        let mut time = world.resource_mut::<Time<Virtual>>();
        time.advance_by(Duration::ZERO);
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
        "note": "Time set. Absolute-time systems observe the new elapsed time on the next frame; delta-accumulated state is not replayed. GlobalTransform synced for spatial queries.",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::ErrorCode;

    fn world_with_virtual_time() -> World {
        let mut world = World::new();
        world.init_resource::<Time<Virtual>>();
        world
    }

    /// `seconds < 0.0` is false for NaN, and Duration saturates near 1.8e19
    /// seconds, so both reached `Duration::from_secs_f64` and panicked the
    /// control plane on an otherwise valid JSON request.
    #[test]
    fn seek_time_rejects_non_finite_and_overflowing_seconds() {
        for seconds in [f64::NAN, f64::INFINITY, 1e30] {
            let mut world = world_with_virtual_time();
            let error = seek_time(
                &mut world,
                SeekTimeParams {
                    seconds,
                    pause: true,
                },
            )
            .expect_err("must be rejected, not panic");
            assert_eq!(error.code, ErrorCode::InvalidParams);
        }
    }

    #[test]
    fn seek_time_still_accepts_a_normal_target() {
        let mut world = world_with_virtual_time();
        let result = seek_time(
            &mut world,
            SeekTimeParams {
                seconds: 2.5,
                pause: true,
            },
        )
        .expect("a finite target must seek");
        assert_eq!(result["elapsed_secs"], 2.5);
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
    fn test_resume_time_snaps_zero_speed_to_one() {
        // If a user system (or external code) sets relative_speed to 0,
        // resume_time must restore playback speed. Otherwise the scene
        // stays frozen with `paused: false` (silent freeze).
        let mut world = world_with_virtual_time();
        {
            let mut time = world.resource_mut::<Time<Virtual>>();
            time.pause();
            time.set_relative_speed(0.0);
        }
        let result = resume_time(&mut world).unwrap();
        assert_eq!(result["paused"], false);
        assert_eq!(result["relative_speed"], 1.0);
    }

    #[test]
    fn test_resume_time_preserves_nonzero_speed() {
        // The snap-to-1.0 behavior must only fire on exact 0; a deliberate
        // slow-mo speed (e.g. 0.25) survives a pause+resume cycle.
        let mut world = world_with_virtual_time();
        {
            let mut time = world.resource_mut::<Time<Virtual>>();
            time.set_relative_speed(0.25);
            time.pause();
        }
        let result = resume_time(&mut world).unwrap();
        assert_eq!(result["paused"], false);
        assert_eq!(result["relative_speed"], 0.25);
    }

    #[test]
    fn test_set_time_scale() {
        let mut world = world_with_virtual_time();
        let result = set_time_scale(&mut world, 2.5).unwrap();
        assert_eq!(result["relative_speed"], 2.5);
    }

    #[test]
    fn test_set_time_scale_rejects_negative() {
        let mut world = world_with_virtual_time();
        let err = set_time_scale(&mut world, -1.0).unwrap_err();
        assert!(err.message.contains("must be > 0"));
        // World still alive (no panic).
        let speed = world.resource::<Time<Virtual>>().relative_speed();
        assert!(speed > 0.0);
    }

    #[test]
    fn test_set_time_scale_rejects_zero() {
        let mut world = world_with_virtual_time();
        let err = set_time_scale(&mut world, 0.0).unwrap_err();
        assert!(err.message.contains("must be > 0"));
    }

    #[test]
    fn test_set_time_scale_rejects_nan() {
        let mut world = world_with_virtual_time();
        let err = set_time_scale(&mut world, f32::NAN).unwrap_err();
        assert!(err.message.contains("finite"));
    }

    #[test]
    fn test_set_time_scale_rejects_infinity() {
        let mut world = world_with_virtual_time();
        let err = set_time_scale(&mut world, f32::INFINITY).unwrap_err();
        assert!(err.message.contains("finite"));
    }

    #[test]
    fn test_set_time_scale_rejects_too_large() {
        // Regression: a huge finite scale passes the > 0 / finite guard but then
        // drives Bevy's fixed-timestep accumulator into a spiral of death,
        // freezing the frame loop for seconds and timing out the control channel.
        let mut world = world_with_virtual_time();
        let err = set_time_scale(&mut world, 1.0e6).unwrap_err();
        assert!(err.message.contains("1000"));
        // World still alive at the prior speed (no mutation, no panic).
        let speed = world.resource::<Time<Virtual>>().relative_speed();
        assert!((speed - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_set_time_scale_accepts_max() {
        // The boundary itself is allowed; only values above it are rejected.
        let mut world = world_with_virtual_time();
        let result = set_time_scale(&mut world, MAX_TIME_SCALE).unwrap();
        assert_eq!(result["relative_speed"], MAX_TIME_SCALE);
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
        let result = seek_time(
            &mut world,
            SeekTimeParams {
                seconds: 5.0,
                pause: false,
            },
        )
        .unwrap();
        let elapsed = result["elapsed_secs"].as_f64().unwrap();
        assert!((elapsed - 5.0).abs() < 0.01);
        assert_eq!(result["paused"], false);
    }

    #[test]
    fn test_seek_time_backward() {
        let mut world = world_with_virtual_time();
        // Seek forward first
        seek_time(
            &mut world,
            SeekTimeParams {
                seconds: 10.0,
                pause: false,
            },
        )
        .unwrap();
        // Then seek backward
        let result = seek_time(
            &mut world,
            SeekTimeParams {
                seconds: 3.0,
                pause: false,
            },
        )
        .unwrap();
        let elapsed = result["elapsed_secs"].as_f64().unwrap();
        assert!((elapsed - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_seek_time_leaves_no_frame_delta() {
        // The fixed main loop consumes Time<Virtual>::delta; a seek that left
        // the jump there would replay it as FixedMain catch-up iterations.
        let mut world = world_with_virtual_time();
        seek_time(
            &mut world,
            SeekTimeParams {
                seconds: 120.0,
                pause: false,
            },
        )
        .unwrap();
        assert_eq!(world.resource::<Time<Virtual>>().delta(), Duration::ZERO);
        // The backward branch rebuilds the clock and advances again.
        seek_time(
            &mut world,
            SeekTimeParams {
                seconds: 30.0,
                pause: false,
            },
        )
        .unwrap();
        let time = world.resource::<Time<Virtual>>();
        assert_eq!(time.delta(), Duration::ZERO);
        assert!((time.elapsed_secs_f64() - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_seek_time_negative_error() {
        let mut world = world_with_virtual_time();
        let result = seek_time(
            &mut world,
            SeekTimeParams {
                seconds: -1.0,
                pause: false,
            },
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::InvalidParams);
    }

    #[test]
    fn test_seek_time_with_pause() {
        let mut world = world_with_virtual_time();
        let result = seek_time(
            &mut world,
            SeekTimeParams {
                seconds: 5.0,
                pause: true,
            },
        )
        .unwrap();
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
        let result = seek_time(
            &mut world,
            SeekTimeParams {
                seconds: 2.0,
                pause: false,
            },
        )
        .unwrap();
        assert!(result["note"].as_str().is_some());
    }
}
