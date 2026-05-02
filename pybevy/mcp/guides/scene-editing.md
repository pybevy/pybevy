# Scene Editing Guide

MCP workflow for spawning, editing, screenshotting, time control, and field value formats.

## MCP Workflow Overview

The typical MCP scene editing workflow:

1. **Pause time** — Freeze the scene for inspection
2. **Inspect** — List entities, query by component
3. **Edit** — Spawn, modify, or remove entities/components
4. **Screenshot** — Capture visual result
5. **Resume** — Unpause to see animation

## Time Control

Control game time without affecting rendering:

```
pause_time           — Freeze all time-dependent systems
resume_time          — Unpause
set_time_scale {"scale": 0.1}  — Slow-motion (0.1 = 10% speed)
set_time_scale {"scale": 2.0}  — Fast-forward (2x speed)
get_time_status      — Check paused state, speed, elapsed time
```

**Key:** Pausing freezes `Time<Virtual>` which controls game logic. Rendering continues so you can still take screenshots of the frozen scene.

**Tip:** You can combine pause and scale — e.g., `pause_time`, then `set_time_scale {"scale": 1.0}` to reset speed while paused, then `resume_time` to continue at the new speed.

### Slow-Motion Capture Technique

Use `schedule_actions` to batch time control + capture atomically (no drift between calls):
```
schedule_actions {"actions": [
    {"tool": "pause_time"},
    {"tool": "set_time_scale", "args": {"scale": 0.1}},
    {"tool": "resume_time"},
    {"tool": "capture_screenshot", "args": {"delay_frames": 60}},
    {"tool": "pause_time"},
    {"tool": "set_time_scale", "args": {"scale": 1.0}},
    {"tool": "resume_time"}
]}
```

Or use `reload` with `pause` and `time_scale` for the initial setup:
```
reload {"mode": "full", "pause": true, "time_scale": 0.1}
```

## Spawning Entities

```
spawn_entity {"components": {
    "Transform": {"translation": [0, 5, 0], "scale": [2, 2, 2]},
    "PointLight": {"intensity": 1000, "color": [1, 0.8, 0.6, 1]},
    "Name": "warm_light"
}}
```

Use `get_component_schema` to discover required fields:
```
get_component_schema {"name": "PointLight"}
→ Shows all fields, types, defaults, and a spawn JSON example
```

## Modifying Components

Partial updates — only specify fields to change:
```
set_component {
    "name": "warm_light",
    "component": "PointLight",
    "fields": {"intensity": 2000, "shadows_enabled": true}
}
```

## Spatial Verification

Check positions, distances, and overlaps between entities without needing screenshots:

```
query_spatial {"name_a": "table", "name_b": "chair"}
→ Distance, direction, AABB overlap/gap between two entities

query_spatial {"name": "player", "radius": 5.0}
→ Find all entities within 5 units of the player

check_overlaps {"name": "lamp"}
→ Check what overlaps with the lamp, detect if it's floating

check_overlaps {}
→ Scene-wide overlap scan: find all clipping pairs + floating entities
```

**Note on hierarchies:** By default (`include_siblings=false`), overlapping entities that share a common root ancestor (e.g., mesh children within the same GLB model) are excluded from results. Set `include_siblings=true` to include intra-model overlaps.

**Bounding boxes** for individual entity measurements:
```
get_bounding_box {"name": "table"}
→ Local + world-space AABB (center, min, max, size)
```

## Live Material Tweaking

Use `get_type_definition(type_name="StandardMaterial")` to see all available fields and defaults.

```
set_asset {"name": "table", "component": "MeshMaterial3d",
    "asset_type": "StandardMaterial", "fields": {"base_color": [1,0,0,1]}}
```

## Camera Control

Find and adjust the camera:
```
query_entities {"with": ["Camera3d", "Transform"]}
→ Get camera entity ID

set_component {
    "entity_id": <camera_id>,
    "component": "Transform",
    "fields": {"translation": [10, 8, 10]}
}
```

## Screenshots

```
capture_screenshot                              — Standard capture (768px wide)
capture_screenshot {"max_width": 1280}          — Higher resolution
capture_screenshot {"delay_frames": 5}          — Wait 5 frames first
capture_screenshot {"gizmos": true}             — With entity ID/Name overlays
```

### One-Shot Edit-and-Verify

Use `reload_and_capture` to reload + check errors + screenshot in a single round-trip:
```
reload_and_capture {"mode": "full", "pause": true}
→ Returns: {reload: {status, errors}, screenshot: <base64>, entity_count}
```
This replaces the 3-step `reload` → `get_logs` → `capture_screenshot` workflow.

### Multi-Angle Inspection

Use `capture_turnaround` to see the scene from all angles in one image:
```
capture_turnaround {}
→ Auto-fits camera distance/center to scene bounds, captures 6 views + top-down

capture_turnaround {"look_at": [0, 1, 0], "distance": 8, "view_count": 8}
→ Custom orbit around a specific point
```

**Tip:** Auto-fit distance can be dominated by large ground planes. For scenes with wide geometry, explicitly supply `look_at` and `distance` parameters rather than relying on auto-fit.

### Depth Probing

Use `capture_depth` to get distance measurements from camera to entities:
```
capture_depth {"position": [5, 5, 5], "look_at": [0, 0, 0], "grid_density": 8}
→ RGB screenshot + 64 ray-AABB depth samples with hit entity IDs and world positions
```

## Batched Schedules

Use `schedule_actions` to execute multiple tool calls inside the engine frame loop with precise timing. Same-`at` actions fire in the same frame (atomic), eliminating drift between sequential tool calls.

### Time-Lapse Capture

Seek to different game times and screenshot without time advancing between captures:
```
schedule_actions {"actions": [
    {"tool": "pause_time"},
    {"tool": "seek_time", "args": {"seconds": 0}, "label": "t0"},
    {"tool": "capture_screenshot", "args": {"max_width": 768}, "label": "shot_t0"},
    {"tool": "seek_time", "args": {"seconds": 5}, "label": "t5"},
    {"tool": "capture_screenshot", "args": {"max_width": 768}, "label": "shot_t5"},
    {"tool": "seek_time", "args": {"seconds": 10}, "label": "t10"},
    {"tool": "capture_screenshot", "args": {"max_width": 768}, "label": "shot_t10"},
    {"tool": "resume_time"}
]}
```

### Atomic Edit + Verify

Mutate an entity and capture in the same batch — no intermediate frames:
```
schedule_actions {"actions": [
    {"tool": "set_component", "args": {"name": "sun", "component": "PointLight", "fields": {"intensity": 5000}}},
    {"tool": "capture_screenshot", "label": "after_edit"}
]}
```

### Time-Offset Actions

Actions at different `at` values fire at the corresponding game-time offset from schedule start:
```
schedule_actions {"actions": [
    {"tool": "capture_screenshot", "at": 0, "label": "start"},
    {"tool": "capture_screenshot", "at": 2, "label": "2sec_later"},
    {"tool": "capture_screenshot", "at": 4, "label": "4sec_later"}
]}
```

### Error Handling

- `stop_on_error: true` — abort remaining actions on first failure
- `skip_if_error: "label"` — skip this action if the labeled action errored

```
schedule_actions {"stop_on_error": true, "actions": [
    {"tool": "set_component", "args": {"name": "player", "component": "Transform", "fields": {"translation": [0, 5, 0]}}, "label": "move"},
    {"tool": "capture_screenshot", "label": "verify", "skip_if_error": "move"}
]}
```

### Async Mode

For long-running schedules, use `mode: "async"` to get an immediate schedule ID, then poll:
```
schedule_actions {"mode": "async", "actions": [...]}
→ {"schedule_id": "schedule-0", "status": "running"}

get_schedule_result {"schedule_id": "schedule-0"}
→ {"status": "completed", "results": [...]}
```

### What Can Be Scheduled

All engine-side tools work: `pause_time`, `resume_time`, `seek_time`, `set_time_scale`, `capture_screenshot`, `capture_turnaround`, `capture_depth`, `capture_timeline`, `set_component`, `remove_component`, `spawn_entity`, `despawn_entity`, `query_entities`, `get_scene_summary`, `query_spatial`, `check_overlaps`, `get_bounding_box`, `get_component`, `set_resource`, `remove_resource`, `run_code`, `batch`, `set_asset`, `get_performance`, `get_time_status`.

**Not schedulable**: `get_logs`, `search_api`, `get_type_definition`, `run_scene`, `get_started` (bridge-local), and `reload`, `reload_and_capture` (the schedule blocks the same frame loop the reload needs to drop and re-enter).

## Iterative Editing

**Make one change at a time**, then screenshot to verify before moving on. Do NOT batch multiple changes into a single edit.

Why this matters:
- Particle sizes, lighting values, and positions are hard to predict — you'll often need to adjust
- A single screenshot catches sizing/positioning bugs immediately
- Batching 5 changes makes it impossible to tell which one caused a visual problem

**Good workflow** (adding smoke + sparkles + fire):
1. Add smoke → `reload` → `capture_screenshot` → adjust size if needed
2. Add sparkles → `reload` → `capture_screenshot` → adjust
3. Add fire → `reload` → `capture_screenshot` → adjust

**Bad workflow**: Add all three at once → screenshot shows something wrong → which one broke?

## Complete Example Session

```
1. pause_time                                    — Freeze scene
2. resources/read scene://entities               — See all entities
3. query_entities {"with": ["PointLight"]}        — Find lights
4. set_component {"name": "sun", ...}             — Adjust light
5. spawn_entity {"components": {...}}             — Add new object
6. capture_screenshot                             — Check result
7. resume_time                                    — Unpause
8. capture_screenshot {"delay_frames": 30}        — See animation
```

## Animation Verification

Use `schedule_actions` to seek to an exact time and inspect deterministically:

```
schedule_actions {"actions": [
    {"tool": "pause_time"},
    {"tool": "seek_time", "args": {"seconds": 2.5}},
    {"tool": "run_code", "args": {"code": "def inspect(query: Query[tuple[Transform, Name]], time: Res[Time]):\n  t = time.elapsed_secs()\n  for tr, name in query:\n    print(f'{name}: y={tr.translation.y:.3f} at t={t:.2f}')\nworld.run_system_once(inspect)"}},
    {"tool": "capture_screenshot", "label": "at_2.5s"},
    {"tool": "resume_time"}
]}
```

Then call `get_logs()` to read the printed values.

**Multi-time comparison** — seek to different times and capture each:

```
schedule_actions {"actions": [
    {"tool": "pause_time"},
    {"tool": "seek_time", "args": {"seconds": 0}, "label": "t0"},
    {"tool": "capture_screenshot", "label": "frame_0"},
    {"tool": "seek_time", "args": {"seconds": 1}, "label": "t1"},
    {"tool": "capture_screenshot", "label": "frame_1"},
    {"tool": "seek_time", "args": {"seconds": 2}, "label": "t2"},
    {"tool": "capture_screenshot", "label": "frame_2"},
    {"tool": "resume_time"}
]}
```

This lets you confirm animations produce correct values at specific moments without timing drift between tool calls.

## Field Value Formats (MCP JSON)

| Type | JSON Format | Example |
|------|-------------|---------|
| Vec2 | `[x, y]` | `[1.0, 2.0]` |
| Vec3 | `[x, y, z]` | `[0, 5, 0]` |
| Vec4 | `[x, y, z, w]` | `[1, 0, 0, 1]` |
| Quat | `[x, y, z, w]` | `[0, 0, 0, 1]` |
| Color | `[r, g, b, a]` | `[1.0, 0.5, 0.0, 1.0]` |
| UVec2/IVec2 | `[x, y]` | `[700, 0]` |
| float | number | `500.0` |
| int/isize | integer | `2` |
| bool | boolean | `true` |
| str | string | `"player"` |
| Option<T> | value or `null` | `[1, 0, 0]` or `null` |
| Vec<f32> | array | `[1.0, 0.5, 0.0, 0.25]` |

### Common Payload Examples

```
# Spawn a light with name
spawn_entity {"components": {
    "PointLight": {"intensity": 5000.0, "color": [1.0, 0.8, 0.6, 1.0], "shadows_enabled": true},
    "Transform": {"translation": [0, 3, 0]},
    "Name": "my_light"
}}

# Update light intensity
set_component {"name": "my_light", "component": "PointLight", "fields": {"intensity": 8000.0}}

# Set camera viewport (Option<Viewport> — omit depth for defaults)
set_component {"name": "my_camera", "component": "Camera",
    "fields": {"viewport": {"physical_position": [700, 0], "physical_size": [580, 720]}}}

# Clear an optional field
set_component {"name": "my_camera", "component": "Camera", "fields": {"viewport": null}}
```

**Note:** `Name` is passed as a bare string, not `{"name": "..."}`. Color is `[r, g, b, a]`.

## MCP Integration

When running with `McpPlugin`, AI agents can inspect and manipulate the scene at runtime. Use `get_component_schema` to inspect any component's fields and JSON spawn format.

```
query_entities {"with": ["Transform", "PointLight"]}
resources/read scene://entity/MainCamera
set_component {"name": "sun", "component": "PointLight", "fields": {"intensity": 2000}}
spawn_entity {"components": {"Transform": {"translation": [0, 1, 0]}, "Player": {"player_id": 1, "health": 100.0}}}
```

## run_code Tips

To access custom scene types inside `run_code`, import the scene module by filename:
```
run_code {"code": "import scene_layers; print(dir(scene_layers))"}
```
The scene's module is importable by its filename stem, not via `__main__`.

`entity_id` from `query_entities` (and other MCP responses) is a raw `u64`. To use it inside `run_code`, convert with `Entity.from_bits` first:
```
run_code {"code": "e = Entity.from_bits(123456); print(world.entity(e))"}
```
`world.entity(int)` is intentionally rejected to mirror Bevy's typed API: raw bits encode generation+index, and silent acceptance risks aliasing recycled entities.

**Note:** `run_code` with `world.run_system_once()` queries against the live world state. Prior MCP mutations (spawn, set_component) are automatically flushed before the system runs, so queries should see all entities.

## Troubleshooting

- **Entity not found**: IDs change after full reload. Use `Name` or re-query.
- **Component not on entity**: Use `scene://entity/{id}` to see what components exist.
- **Field update fails**: Check field name/type with `get_component_schema`. Use `get_component_schema` to check the `editable` flag — some components (like `Text`) require code-side updates rather than MCP field mutation.
- **Screenshot blank**: Wait more frames with `delay_frames` parameter.
- **Entity counts differ**: `get_scene_summary` counts user-visible entities only. `get_performance` reports the full ECS entity count including render internals, so the two numbers will differ.
