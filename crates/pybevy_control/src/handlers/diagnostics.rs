use bevy::{ecs::world::World, time::Time};

use crate::bridge::ControlError;

/// Get performance metrics from the debug overlay snapshot.
/// Falls back to basic Time-based metrics if DebugSnapshot is not populated.
pub fn get_performance(world: &mut World) -> Result<serde_json::Value, ControlError> {
    // Live each call, not from the 1 Hz snapshot, so it never lags spawns.
    let entity_count = super::entity_count::scene_entity_count(world);

    // Try rich snapshot first (populated by hot reload overlay system)
    if let Some(snap) = world.get_resource::<pybevy_core::DebugSnapshot>()
        && snap.populated
    {
        let mut result = serde_json::Map::new();

        // Performance
        result.insert("fps_average".into(), serde_json::json!(snap.fps_average));
        result.insert("fps_current".into(), serde_json::json!(snap.fps_current));
        result.insert("uptime_secs".into(), serde_json::json!(snap.uptime_secs));
        result.insert(
            "generation_uptime_secs".into(),
            serde_json::json!(snap.generation_uptime_secs),
        );

        // Scene: live count, matches list_entities / scene_summary.
        result.insert("entity_count".into(), serde_json::json!(entity_count));
        result.insert("entity_count_scope".into(), serde_json::json!("all"));
        if !snap.asset_counts.is_empty() {
            let assets: serde_json::Map<_, _> = snap
                .asset_counts
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::json!(v)))
                .collect();
            result.insert("assets".into(), serde_json::Value::Object(assets));
        }

        // System resources
        result.insert("memory_mb".into(), serde_json::json!(snap.memory_mb));
        result.insert(
            "total_memory_mb".into(),
            serde_json::json!(snap.total_memory_mb),
        );
        result.insert("cpu_percent".into(), serde_json::json!(snap.cpu_percent));
        result.insert("cpu_cores".into(), serde_json::json!(snap.cpu_core_count));

        // Reload info
        result.insert("reload_count".into(), serde_json::json!(snap.reload_count));
        if let Some(ref mode) = snap.last_reload_mode {
            result.insert("last_reload_mode".into(), serde_json::json!(mode));
        }
        if snap.reload_failed {
            result.insert("reload_failed".into(), serde_json::json!(true));
            if let Some(ref reason) = snap.reload_failure_reason {
                result.insert("reload_failure_reason".into(), serde_json::json!(reason));
            }
        }

        // Python info
        result.insert("gil_enabled".into(), serde_json::json!(snap.gil_enabled));

        // System profiling. `max_ms` is the rolling-window peak; useful
        // because `avg_ms` smooths away single-frame spikes.
        if !snap.update_profiles.is_empty() {
            let profiles: Vec<_> = snap
                .update_profiles
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "name": p.name,
                        "avg_ms": format!("{:.2}", p.avg_ms),
                        "max_ms": format!("{:.2}", p.max_ms),
                    })
                })
                .collect();
            result.insert("system_profiles".into(), serde_json::json!(profiles));
        }
        if !snap.startup_profiles.is_empty() {
            let profiles: Vec<_> = snap
                .startup_profiles
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "name": p.name,
                        "avg_ms": format!("{:.2}", p.avg_ms),
                        "max_ms": format!("{:.2}", p.max_ms),
                    })
                })
                .collect();
            result.insert("startup_profiles".into(), serde_json::json!(profiles));
        }

        // Memory profiling
        if snap.total_schedule_systems > 0 {
            result.insert(
                "total_schedule_systems".into(),
                serde_json::json!(snap.total_schedule_systems),
            );
            // Live scene systems for the current generation. Reported alongside
            // the total so a rising total across reloads reads as expected
            // generation accumulation (inert prior-generation systems), not a leak.
            result.insert(
                "current_generation_systems".into(),
                serde_json::json!(snap.current_generation_systems),
            );
        }
        if snap.python_gc_objects > 0 {
            result.insert(
                "python_gc_objects".into(),
                serde_json::json!(snap.python_gc_objects),
            );
        }
        if snap.memory_growth_mb != 0.0 {
            result.insert(
                "memory_growth_mb".into(),
                serde_json::json!(format!("{:.1}", snap.memory_growth_mb)),
            );
        }
        if snap.memory_peak_mb > 0.0 {
            result.insert(
                "memory_peak_mb".into(),
                serde_json::json!(format!("{:.1}", snap.memory_peak_mb)),
            );
        }
        if snap.memory_warning {
            result.insert("memory_warning".into(), serde_json::json!(true));
        }
        if !snap.reload_memory_snapshots.is_empty() {
            let snapshots: Vec<_> = snap
                .reload_memory_snapshots
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "generation": s.generation,
                        "rss_mb": format!("{:.1}", s.rss_mb),
                        "delta_mb": format!("{:.1}", s.delta_mb),
                        "gc_objects": s.gc_objects,
                        "schedule_systems": s.schedule_systems,
                        "current_generation_systems": s.current_generation_systems,
                    })
                })
                .collect();
            result.insert(
                "reload_memory_snapshots".into(),
                serde_json::json!(snapshots),
            );
        }

        return Ok(serde_json::Value::Object(result));
    }

    // Fallback: basic metrics from Time resource (entity_count already live from above)
    let mut result = serde_json::Map::new();
    result.insert("entity_count".into(), serde_json::json!(entity_count));
    result.insert("entity_count_scope".into(), serde_json::json!("all"));

    if let Some(time) = world.get_resource::<Time>() {
        let delta_secs = time.delta_secs_f64();
        result.insert(
            "frame_time_ms".into(),
            serde_json::json!(delta_secs * 1000.0),
        );
        if delta_secs > 0.0 {
            result.insert("fps".into(), serde_json::json!(1.0 / delta_secs));
        }
        result.insert(
            "elapsed_secs".into(),
            serde_json::json!(time.elapsed_secs_f64()),
        );
    }

    Ok(serde_json::Value::Object(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_performance_empty_world() {
        let mut world = World::new();
        let result = get_performance(&mut world).unwrap();
        assert_eq!(result["entity_count"], 0);
        // No Time resource, so no fps/frame_time
        assert!(result.get("fps").is_none());
    }

    #[test]
    fn get_performance_with_entities() {
        let mut world = World::new();
        world.spawn_empty();
        world.spawn_empty();
        world.spawn_empty();
        let result = get_performance(&mut world).unwrap();
        // world.entities().len() may include internal ECS placeholders
        assert!(result["entity_count"].as_u64().unwrap() >= 3);
    }

    #[test]
    fn get_performance_with_populated_snapshot_reads_snapshot_for_metrics_but_live_for_entity_count()
     {
        // Snapshot is read for FPS / RAM (genuinely expensive to compute live),
        // but entity_count must come from a live query so it's never stale.
        // The 1 Hz overlay snapshot otherwise caches entity_count and masks
        // entities spawned between ticks.
        let mut world = World::new();
        let mut snap = pybevy_core::DebugSnapshot::default();
        snap.populated = true;
        snap.fps_average = 60.0;
        snap.fps_current = 59.5;
        snap.entity_count = 42; // stale snapshot value; should be ignored
        snap.memory_mb = 128.0;
        snap.uptime_secs = 10.0;
        snap.generation_uptime_secs = 4.0;
        world.insert_resource(snap);

        // World has 0 live entities. Even with a snapshot saying 42,
        // get_performance must report the live count.
        let result = get_performance(&mut world).unwrap();
        assert_eq!(result["fps_average"], 60.0);
        assert_eq!(result["memory_mb"], 128.0);
        assert_eq!(result["uptime_secs"], 10.0);
        assert_eq!(result["generation_uptime_secs"], 4.0);
        assert_eq!(
            result["entity_count"], 0,
            "entity_count must be live, not from the cached snapshot"
        );
    }

    #[test]
    fn get_performance_entity_count_reflects_live_spawns_with_snapshot() {
        // Spawning entities should be reflected in get_performance.entity_count
        // even when the overlay snapshot hasn't ticked since spawn.
        let mut world = World::new();
        let mut snap = pybevy_core::DebugSnapshot::default();
        snap.populated = true;
        snap.entity_count = 0; // pre-spawn snapshot
        world.insert_resource(snap);

        for _ in 0..7 {
            world.spawn_empty();
        }

        let result = get_performance(&mut world).unwrap();
        assert_eq!(result["entity_count"], 7);
    }

    #[test]
    fn get_performance_unpopulated_snapshot_uses_fallback() {
        let mut world = World::new();
        let snap = pybevy_core::DebugSnapshot::default(); // populated is false by default
        world.insert_resource(snap);
        let result = get_performance(&mut world).unwrap();
        // Should fall through to basic metrics (entity_count from world, not snapshot)
        assert!(result["entity_count"].is_number());
        assert!(result.get("fps_average").is_none());
    }

    #[test]
    fn get_performance_with_time_resource() {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        let result = get_performance(&mut world).unwrap();
        assert!(result["entity_count"].is_number());
        assert!(result.get("elapsed_secs").is_some());
    }

    #[test]
    fn get_performance_rich_snapshot_all_fields() {
        let mut world = World::new();
        let mut snap = pybevy_core::DebugSnapshot::default();
        snap.populated = true;
        snap.fps_average = 60.0;
        snap.fps_current = 59.0;
        snap.uptime_secs = 100.0;
        snap.generation_uptime_secs = 12.0;
        snap.entity_count = 50;
        snap.memory_mb = 256.0;
        snap.total_memory_mb = 512.0;
        snap.cpu_percent = 25.0;
        snap.cpu_core_count = 8;
        snap.gil_enabled = true;
        snap.reload_count = 3;
        snap.last_reload_mode = Some("full".to_string());
        snap.reload_failed = true;
        snap.reload_failure_reason = Some("syntax error".to_string());
        snap.asset_counts = vec![("Mesh".to_string(), 5), ("Material".to_string(), 3)];
        snap.update_profiles = vec![pybevy_core::SystemProfile {
            name: "my_system".to_string(),
            avg_ms: 1.5,
            max_ms: 4.2,
        }];
        snap.startup_profiles = vec![pybevy_core::SystemProfile {
            name: "setup".to_string(),
            avg_ms: 10.0,
            max_ms: 12.5,
        }];
        snap.total_schedule_systems = 12;
        snap.current_generation_systems = 6;
        snap.python_gc_objects = 5000;
        snap.memory_growth_mb = 8.5;
        snap.memory_peak_mb = 300.0;
        snap.memory_warning = true;
        snap.reload_memory_snapshots = vec![pybevy_core::ReloadMemorySnapshotInfo {
            generation: 1,
            rss_mb: 250.0,
            delta_mb: 10.0,
            gc_objects: 4000,
            schedule_systems: 10,
            current_generation_systems: 6,
        }];
        world.insert_resource(snap);

        let result = get_performance(&mut world).unwrap();
        assert_eq!(result["fps_average"], 60.0);
        assert_eq!(result["uptime_secs"], 100.0);
        assert_eq!(result["generation_uptime_secs"], 12.0);
        // entity_count is live, not from the snapshot. World has no entities
        // here, so the response is 0 even with snap.entity_count=50.
        assert_eq!(result["entity_count"], 0);
        assert_eq!(result["reload_count"], 3);
        assert_eq!(result["last_reload_mode"], "full");
        assert_eq!(result["reload_failed"], true);
        assert_eq!(result["reload_failure_reason"], "syntax error");
        assert!(result["assets"].is_object());
        assert!(result["system_profiles"].is_array());
        assert!(result["startup_profiles"].is_array());
        // System profile entries carry both avg and max (rolling-window peak).
        assert_eq!(result["system_profiles"][0]["name"], "my_system");
        assert_eq!(result["system_profiles"][0]["avg_ms"], "1.50");
        assert_eq!(result["system_profiles"][0]["max_ms"], "4.20");
        assert_eq!(result["startup_profiles"][0]["max_ms"], "12.50");
        assert_eq!(result["total_schedule_systems"], 12);
        assert_eq!(result["current_generation_systems"], 6);
        assert_eq!(
            result["reload_memory_snapshots"][0]["current_generation_systems"],
            6
        );
        assert_eq!(result["python_gc_objects"], 5000);
        assert!(result["memory_growth_mb"].is_string());
        assert!(result["memory_peak_mb"].is_string());
        assert_eq!(result["memory_warning"], true);
        assert!(result["reload_memory_snapshots"].is_array());
        assert_eq!(result["gil_enabled"], true);
        assert_eq!(result["cpu_cores"], 8);
    }
}
