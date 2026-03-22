use bevy::{ecs::world::World, time::Time};

use crate::bridge::ControlError;

/// Get performance metrics from the debug overlay snapshot.
/// Falls back to basic Time-based metrics if DebugSnapshot is not populated.
pub fn get_performance(world: &mut World) -> Result<serde_json::Value, ControlError> {
    // Try rich snapshot first (populated by hot reload overlay system)
    if let Some(snap) = world.get_resource::<pybevy_core::DebugSnapshot>() {
        if snap.populated {
            let mut result = serde_json::Map::new();

            // Performance
            result.insert("fps_average".into(), serde_json::json!(snap.fps_average));
            result.insert("fps_current".into(), serde_json::json!(snap.fps_current));
            result.insert("uptime_secs".into(), serde_json::json!(snap.uptime_secs));

            // Scene — includes all ECS entities (framework + user-authored)
            result.insert("entity_count".into(), serde_json::json!(snap.entity_count));
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

            // System profiling
            if !snap.update_profiles.is_empty() {
                let profiles: Vec<_> = snap.update_profiles.iter()
                    .map(|(name, ms)| serde_json::json!({"name": name, "avg_ms": format!("{:.2}", ms)}))
                    .collect();
                result.insert("system_profiles".into(), serde_json::json!(profiles));
            }
            if !snap.startup_profiles.is_empty() {
                let profiles: Vec<_> = snap.startup_profiles.iter()
                    .map(|(name, ms)| serde_json::json!({"name": name, "avg_ms": format!("{:.2}", ms)}))
                    .collect();
                result.insert("startup_profiles".into(), serde_json::json!(profiles));
            }

            // Memory profiling
            if snap.total_schedule_systems > 0 {
                result.insert(
                    "total_schedule_systems".into(),
                    serde_json::json!(snap.total_schedule_systems),
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
    }

    // Fallback: basic metrics from Time resource
    let mut result = serde_json::Map::new();
    let entity_count = world.entities().len() as u64;
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
