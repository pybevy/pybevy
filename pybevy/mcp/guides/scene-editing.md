# Scene Editing Guide

MCP workflow for spawning, editing, screenshotting, time control, and field value formats.

## MCP Workflow Overview

The typical MCP scene editing workflow:

1. **Pause time** - Freeze the scene for inspection
2. **Inspect** - List entities, query by component
3. **Edit** - Spawn, modify, or remove entities/components
4. **Screenshot** - Capture visual result
5. **Resume** - Unpause to see animation

## Time Control

Control game time without affecting rendering:

```
pause_time           - Freeze all time-dependent systems
resume_time          - Unpause
set_time_scale {"scale": 0.1}  - Slow-motion (0.1 = 10% speed)
set_time_scale {"scale": 2.0}  - Fast-forward (2x speed)
get_time_status      - Check paused state, speed, elapsed time
```

**Key:** Pausing freezes `Time[Virtual]` which controls game logic. Rendering continues so you can still take screenshots of the frozen scene.

**Tip:** You can combine pause and scale - e.g., `pause_time`, then `set_time_scale {"scale": 1.0}` to reset speed while paused, then `resume_time` to continue at the new speed.

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

Partial updates - only specify fields to change:
```
set_component {
    "entity": "warm_light",
    "component": "PointLight",
    "fields": {"intensity": 2000, "shadow_maps_enabled": true}
}
```

If the component is absent and supports insertion, `set_component` inserts it.

Non-finite float fields use the JSON strings `"NaN"`, `"Infinity"`, and
`"-Infinity"`. Values returned by `get_component` can be passed back to
`set_component` unchanged.

A request that applies nothing fails with 400 rather than reporting success:
a rejected field value, or a spawn whose components could not be built, is an
error status, not a 200 with an `errors` array to notice. Field values are
converted before any are written, so a bad value leaves the component
untouched. A 200 carrying `errors` therefore means a later write failed after
earlier ones landed; `new_values` reports exactly what was applied.

Enum-backed components expose a synthetic `variant` field. Read the current
value with `get_component`, then replace it by name:

```
set_component {
    "entity": "player",
    "component": "Visibility",
    "fields": {"variant": "Hidden"}
}
```

## Parent-Child Hierarchies

The hierarchy is two ordinary components. `ChildOf` carries the parent id in
`value`, and `Children` lists the members in `items`.

```
get_component {"entity": "turret", "component": "ChildOf"}
    -> {"fields": {"value": 4294967245}}
get_component {"entity": "tank", "component": "Children"}
    -> {"fields": {"items": [4294967245, 4294967246]}}
```

Attach or reparent by setting `ChildOf`, and detach with `remove_component`:

```
set_component {
    "entity": "turret",
    "component": "ChildOf",
    "fields": {"value": 4294967244}
}
```

Bevy's relationship hooks maintain `Children` on both the old and the new
parent, so one call moves both ends. `Children` itself is read-only: it reports
`editable: false` and is maintained from the `ChildOf` side.

An empty `Children` is removed rather than left empty, so `get_component` on a
parent with no children returns 404 instead of an empty list.

Resource entities cannot take part in a hierarchy. Bevy despawns children with
their parent, so allowing it would let an unrelated despawn discard a resource.

## Spatial Verification

Check positions, distances, and overlaps between entities without needing screenshots:

```
query_spatial {"entity_a": "table", "entity_b": "chair"}
→ Distance, direction, AABB overlap/gap between two entities

query_spatial_neighborhood {"entity": "player", "radius": 5.0}
→ Find all entities within 5 units of the player

check_overlaps {"entity": "lamp"}
→ Check what overlaps with the lamp, detect if it's floating

check_all_overlaps {}
→ Scene-wide overlap scan: find all clipping pairs + floating entities
```

**Note on hierarchies:** By default (`include_siblings=false`), overlapping entities that share a common root ancestor (e.g., mesh children within the same GLB model) are excluded from results. Set `include_siblings=true` to include intra-model overlaps.

**Bounding boxes** for individual entity measurements:
```
get_bounding_box {"entity": "table"}
→ Local + world-space AABB (center, min, max, size)
```

## Live Material Tweaking

Use `get_type_definition(type_name="StandardMaterial")` to see all available fields and defaults.

```
set_asset {"entity": "table", "component": "MeshMaterial3d",
    "asset_type": "StandardMaterial", "fields": {"base_color": [1,0,0,1]}}
```

## Camera Control

Find and adjust the camera:
```
query_entities {"with": ["Camera3d", "Transform"]}
→ Get camera entity ID

set_component {
    "entity": <camera_id>,
    "component": "Transform",
    "fields": {"translation": [10, 8, 10]}
}
```

## Screenshots

```
capture_screenshot                              - Standard capture (768px wide)
capture_screenshot {"max_width": 1280}          - Higher resolution
capture_screenshot {"delay_frames": 5}          - Wait 5 frames first
capture_screenshot {"gizmos": true}             - Include gizmos
capture_screenshot {"entity": "Robot"}          - Isolate one entity subtree
capture_timeline {"hide_ui": false, "gizmos": true} - Keep authored UI and gizmos
```

Draw debug lines from regular systems with the `Gizmos` system parameter:

```python
from pybevy.prelude import Color, Gizmos, Vec3

def draw_axes(gizmos: Gizmos) -> None:
    gizmos.ray(Vec3.ZERO, Vec3.X, Color.srgb(1.0, 0.0, 0.0))
    gizmos.ray(Vec3.ZERO, Vec3.Y, Color.srgb(0.0, 1.0, 0.0))
```

Add `ShowAabbGizmo()` to one entity to visualize its bounding box. Add
`ShowLightGizmo()` to a light entity; pass `LightGizmoColor.Manual(color)` or
another `LightGizmoColor` strategy when the default light color is unsuitable.
See `guide://gizmos` for the complete drawing API and setup requirements.

Successful screenshots include a `frame_id` in their metadata. Use numeric
analysis when an image is unnecessary:

```
capture_stats {"grid": 4, "sample_points": [[384, 384]]}
capture_stats {"entity": "Robot", "grid": 4}
compare_frames {"a": "<first frame_id>", "b": "<second frame_id>"}
```

Passing `entity` isolates that entity and its descendants while retaining the
active camera, lights, probes, and the camera clear color. Statistics therefore
describe the isolated render, including its background; they are not a
segmentation mask over only the entity's covered pixels.

`capture_stats` downsamples in linear light, then reports display-sRGB channel
means and linear-light luma statistics, a 16-bucket luma histogram, row-major
grid cells, and requested pixel samples. `region` and `sample_points` use pixels
in the resized capture; `max_width` therefore affects their coordinate space.
`health_hints` are heuristics, not render errors.

`compare_frames` reports normalized channel differences, changed pixels as a
percentage from 0 to 100, and the changed bounding box and centroid. Its
`identical` field is true when no pixel exceeds `epsilon`. It requires frames
with equal dimensions. Captures are retained in memory for comparison with a
default limit of eight frames and 32 MiB; old IDs may be evicted.
Pause deterministic scenes for A/B captures. Animation, temporal antialiasing,
auto exposure, particles, and dithering may produce legitimate differences;
use a nonzero `epsilon` when those effects cannot be disabled.

### One-Shot Edit-and-Verify

Use `reload_and_capture` to reload + check errors + screenshot in a single round-trip:
```
reload_and_capture {"mode": "full", "pause": true}
→ Returns: {reload: {status, errors}, screenshot: <base64>, entity_count}
```
This replaces the 3-step `reload` → `get_last_error` → `capture_screenshot` workflow.

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

Auto-grid samples report `grid: [column, row]` with
`coordinate_space: "grid_indices"`. When `sample_points` is provided, samples
instead report `pixel: [x, y]` with `coordinate_space: "pixels_800x800"`.

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

Mutate an entity and capture in the same batch - no intermediate frames:
```
schedule_actions {"actions": [
    {"tool": "set_component", "args": {"entity": "sun", "component": "PointLight", "fields": {"intensity": 5000}}},
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

- `stop_on_error: true` - abort remaining actions on first failure
- `skip_if_error: "label"` - skip this action if the labeled action errored

```
schedule_actions {"stop_on_error": true, "actions": [
    {"tool": "set_component", "args": {"entity": "player", "component": "Transform", "fields": {"translation": [0, 5, 0]}}, "label": "move"},
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

`completed_actions` counts all terminal results, including actions that were
skipped, aborted by `stop_on_error`, or cancelled. Use `executed_actions`,
`skipped_actions`, `aborted_actions`, and `cancelled_actions` for the explicit
breakdown; an executed action may itself have an `error` or `partial` result.

### What Can Be Scheduled

Engine-side tools can be scheduled, including:

- Capture and comparison: `capture_screenshot`, `capture_stats`, `compare_frames`, `capture_turnaround`, `capture_depth`, `capture_timeline`
- Time control: `pause_time`, `resume_time`, `seek_time`, `set_time_scale`, `get_time_status`
- Scene inspection: `query_entities`, `get_scene_summary`, `query_spatial`, `query_spatial_neighborhood`, `check_overlaps`, `check_all_overlaps`, `get_bounding_box`, `get_component`, `get_component_schema`, `get_registry`, `get_last_error`, `get_performance`, `get_reload_status`
- Mutation and code: `set_component`, `remove_component`, `spawn_entity`, `despawn_entity`, `set_resource`, `remove_resource`, `set_asset`, `batch`, `run_code`

**Not schedulable**: `get_logs`, `search_api`, `get_type_definition`, `run_scene`, `get_started` (bridge-local), and `reload`, `reload_and_capture` (the schedule blocks the same frame loop the reload needs to drop and re-enter).

## Iterative Editing

**Make one change at a time**, then screenshot to verify before moving on. Do NOT batch multiple changes into a single edit.

Why this matters:
- Particle sizes, lighting values, and positions are hard to predict - you'll often need to adjust
- A single screenshot catches sizing/positioning bugs immediately
- Batching 5 changes makes it impossible to tell which one caused a visual problem

**Good workflow** (adding smoke + sparkles + fire):
1. Add smoke → `reload` → `capture_screenshot` → adjust size if needed
2. Add sparkles → `reload` → `capture_screenshot` → adjust
3. Add fire → `reload` → `capture_screenshot` → adjust

**Bad workflow**: Add all three at once → screenshot shows something wrong → which one broke?

## Complete Example Session

```
1. pause_time                                    - Freeze scene
2. resources/read scene://entities               - See all entities
3. query_entities {"with": ["PointLight"]}        - Find lights
4. set_component {"entity": "sun", ...}           - Adjust light
5. spawn_entity {"components": {...}}             - Add new object
6. capture_screenshot                             - Check result
7. resume_time                                    - Unpause
8. capture_screenshot {"delay_frames": 30}        - See animation
```

## Animation Verification

Use `schedule_actions` to seek to an exact time and inspect deterministically:

```
schedule_actions {"actions": [
    {"tool": "pause_time"},
    {"tool": "seek_time", "args": {"seconds": 2.5}},
    {"tool": "run_code", "args": {"code": "from pybevy.prelude import Name, Query, Time, Transform\nt = world.resource(Time).elapsed_secs()\nfor tr, name in world.query(Query[tuple[Transform, Name]]):\n  print(f'{name}: y={tr.translation.y:.3f} at t={t:.2f}')"}},
    {"tool": "capture_screenshot", "label": "at_2.5s"},
    {"tool": "resume_time"}
]}
```

Read the `stdout` field in the `run_code` action result returned by
`schedule_actions`. Direct scene and system `print()` output is available from
the MCP `get_logs` tool, but `run_code` captures its own stdout and returns it
inline.

**Multi-time comparison** - seek to different times and capture each:

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
`seek_time` changes absolute virtual elapsed time; it does not simulate the
intervening frames. Systems that build state by accumulating `delta_secs()` do
not jump to the requested moment.

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
    "PointLight": {"intensity": 5000.0, "color": [1.0, 0.8, 0.6, 1.0], "shadow_maps_enabled": true},
    "Transform": {"translation": [0, 3, 0]},
    "Name": "my_light"
}}

# Update light intensity
set_component {"entity": "my_light", "component": "PointLight", "fields": {"intensity": 8000.0}}

# Set camera viewport (Option<Viewport> - omit depth for defaults)
set_component {"entity": "my_camera", "component": "Camera",
    "fields": {"viewport": {"physical_position": [700, 0], "physical_size": [580, 720]}}}

# Clear an optional field
set_component {"entity": "my_camera", "component": "Camera", "fields": {"viewport": null}}
```

**Note:** `Name` is passed as a bare string, not `{"name": "..."}`. Color is `[r, g, b, a]`.

## MCP Integration

When running with `McpPlugin`, AI agents can inspect and manipulate the scene at runtime. Use `get_component_schema` to inspect any component's fields and JSON spawn format.

```
query_entities {"with": ["Transform", "PointLight"]}
resources/read scene://entity/MainCamera
set_component {"entity": "sun", "component": "PointLight", "fields": {"intensity": 2000}}
spawn_entity {"components": {"Transform": {"translation": [0, 1, 0]}, "Name": "player"}}
```

## run_code Tips

`run_code` injects `world`, plus unambiguous custom component and resource types
registered by the scene. It does **not** automatically import PyBevy's built-in
types. Import every built-in used by the snippet from its public module (the
prelude is usually the shortest option):
```
run_code {"code": "from pybevy.prelude import Name, Query, Transform\nfor transform, name in world.query(Query[tuple[Transform, Name]]):\n  print(name, transform.translation)"}
```

To access custom scene types inside `run_code`, import the scene module by filename:
```
run_code {"code": "import scene_layers; print(dir(scene_layers))"}
```
The scene's module is importable by its filename stem, not via `__main__`.

`entity_id` from `query_entities` (and other MCP responses) is a raw `u64`. To use it inside `run_code`, convert with `Entity.from_bits` first:
```
run_code {"code": "from pybevy.prelude import Entity\ne = Entity.from_bits(123456)\nprint(world.entity(e))"}
```
`world.entity(int)` is intentionally rejected to mirror Bevy's typed API: raw bits encode generation+index, and silent acceptance risks aliasing recycled entities.

Use `world.query(Query[...])` for ad-hoc access to live component data. It
supports the same tuple, `Mut`, and filter syntax as a system query. Prior MCP
mutations are flushed before `run_code`, so the query sees the current world.

**Messages cannot be registered from `run_code`.** `World` exposes `register_resource` and `register_component`, but there is no `world.register_message()`; `app.add_message(MyMessage)` creates the App-local channel identity and scheduler metadata. To add a new message type, edit the scene's `@entrypoint` and reload. Once registered, write a message from `run_code` with `world.write_message(MyMessage(...))`. See `guide://patterns` (Messages section) for details.

## Troubleshooting

- **Entity not found**: IDs change after full reload. Use `Name` or re-query.
- **Component not on entity**: Use `scene://entity/{id}` to see what components exist.
- **Field update fails**: Check the field name, type, and `editable` flag with
  `get_component_schema`. Components marked `editable: false` require a
  code-side update; `Text` and `Text2d` expose editable content fields.
- **Screenshot blank**: Wait more frames with `delay_frames` parameter.
- **Entity counts differ between calls**: `get_scene_summary` and
  `get_performance` use the same live scene-entity count and exclude resource
  entities. A difference means the scene changed between the two calls.
