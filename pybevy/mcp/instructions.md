# PyBevy MCP - Python bindings for the Bevy game engine

## Workflow

**Guide URIs are MCP resources, not files.** Call `get_guide("patterns")` (or another name returned by `get_guide("index")`) in every client. You may instead read `guide://...` through the client's MCP resource capability. Some clients also generate resource aliases such as `get_guide_patterns`, but those aliases are optional. Never pass a `guide://` URI to a filesystem `read`, `open`, or shell command.

1. **Read guides iteratively, not all upfront.** Start with `get_guide("index")` to see all available guides, then call `get_guide("patterns")` + ONE relevant recipe (e.g. `recipes/game-logic`, `recipes/outdoor`). That's enough to write the initial scene. Then read additional topic guides (lighting, materials, shadows, etc.) as you add those features in later iterations. Max 2-3 guides before first `run_scene`.
2. **When suggesting scenes** - propose ideas that showcase PyBevy's documented features: emissive bloom, glass/transmission materials, volumetric fog, parent-child hierarchy, day/night cycles. Call `get_guide("index")` to see what's available, then suggest scenes that use those features.
3. **Run scene before scene tools** - `run_scene` must be called before any scene tool (get_component_schema, capture_screenshot, query_entities, etc.). It waits for the scene's control server and first-frame system-error reporting, up to 60 seconds. Startup readiness does not imply asynchronous assets have loaded or a GPU readback frame is available; observe those separately.
4. **Read topic guides before API lookups** - Before using `search_api` or `get_type_definition`, call `get_guide("index")` and check for a relevant curated guide. Guides are faster and more reliable than raw API exploration. Only fall back to API lookups for specifics not covered in guides.
5. **Use API lookup tools for specifics** - If you know the class name, use `get_type_definition('ClassName')` directly for the full definition. Ambiguous short names return qualified candidates; retry with one such as `get_type_definition('image.Image')`. If you don't know the name, use `search_api('keyword')` first, then `get_type_definition` on the results. API lookup covers public stubs and explicitly re-exported pure-Python modules; implementation-only modules are intentionally absent. Search also excludes private class/function bodies and helper-reference lines while preserving original source line numbers.
6. **Reuse the running scene for ordinary edits** - after the first `run_scene`, edit the .py file and call `reload`. Call `run_scene` again when switching scene files, adding or removing bridge-backed plugins, changing core plugin composition, or requiring a clean restart.
7. **After reload** - call `get_last_error` to check the live Python system-error channel. `reload.error` is sampled once before the response and may miss errors reported on a later frame. Use `get_logs(errors_only=true)` as a secondary check for Bevy, asset, render, and subprocess errors.
8. **Scene style defaults (3D)** - new 3D scenes MUST include: Bloom on the camera, DistanceFog for atmospheric depth, a warm key directional light (shadows) + cool fill directional light (no shadows, ~40% intensity), ClearColor matching fog color, and lighting above the minimum floors (see Scene Generation below). For 2D scenes, see `guide://2d`. Start bright, dim later.
9. **Interior camera placement** - for enclosed scenes, use `get_bounding_box` on walls/objects to calculate safe debug camera positions instead of guessing coordinates.
10. **Use `get_guide("scene-quality")` as a reference** - consult its lighting floors and color palette rules when refining visuals, not necessarily before writing the first line of code.
11. **After first load with GLB models** - run `check_all_overlaps(ground_y=0)` to detect sunken models scene-wide in one pass, before any visual iteration.

After an early subprocess exit, `get_logs(lines=200)` retrieves more retained
output than the startup error includes. Retrieve it before starting another scene.

After a failed reload, `run_code` uses the restored previous scene namespace.
Fix the source and reload again to apply the new definitions.

## API Lookup Guide

| Need | Tool | Example |
|------|------|---------|
| Know the class name | `get_type_definition` | `get_type_definition("PointLight")` → full constructor + methods |
| Don't know the name | `search_api` → then `get_type_definition` | `search_api("fog")` → find `DistanceFog` → `get_type_definition("DistanceFog")` |
| Need JSON spawn format | `get_component_schema` | `get_component_schema("Transform")` → field names + defaults |
| Curated topic knowledge | `get_guide` or `guide://` resources | `get_guide("lighting")`, `guide://camera`, `get_guide("patterns")` |

**IMPORTANT**: `search_api` returns brief matches. Always follow up with `get_type_definition` for the full class definition including constructor parameters.

## Key Concepts

- **Entities** have numeric IDs that may change across hot reloads. Prefer Name-based addressing when possible.
- **Components** are data attached to entities (Transform, PointLight, etc.)
- **Resources** are global singletons (Time, AssetServer, etc.)
- **Single** uses `into_inner()` for typed component access and tuple unpacking. Use `Mut[T]` for writes.
- **Systems** are functions that run each frame, organized by Stage (Startup, Update, Last, etc.)
- **Messages** are App-local buffered channels registered with `app.add_message(T)`. Ordered readers can observe same-pass writes. A system may have multiple readers for one channel, but cannot combine a writer with another reader or writer for that same channel.
- **Coordinate system** - Bevy is right-handed, Y-up. Camera default forward is −Z. When a camera on the −Z side looks toward +Z, the X-axis appears mirrored on screen (world +X = screen left). Plan grid layouts accordingly.

## Spatial Intelligence Tools

- `query_spatial` - Pairwise distance/direction/overlap between two entities (`entity_a`, `entity_b`).
- `query_spatial_neighborhood` - Find all entities within `radius` of a center `entity`.
- `check_overlaps` - AABB overlap + floating/sunken detection for one `entity` against all others. Use `ground_y` to flag models sunk below a ground plane.
- `check_all_overlaps` - Scene-wide AABB overlap + floating/sunken detection across every entity. Use `ground_y` for first-load GLB sweeps.
- `reload_and_capture` - One round-trip: reload → error check → render-pipeline readiness → screenshot. The capture delay starts after readiness; asynchronous asset loads may still need more time.
- `capture_turnaround` - Multi-viewpoint orbit capture composited into one contact sheet. Auto-fits to scene bounds.
- `capture_timeline` - Frames over time in a contact sheet. Headless image-target captures apply UI/overlay suppression from the first tile and restore capture state after completion or timeout.
- `capture_depth` - RGB screenshot + ray-AABB depth samples through the selected `Camera3d` projection. Returns **entity names at each sample point** (semantic segmentation), making it the primary tool for diagnosing **occlusion, visibility, and "wrong entity showing"** problems. Use it before repeated screenshots when geometry appears wrong.
- `capture_stats` - Numeric RGB/luma summaries, grid cells, and pixel samples without returning a PNG. Captures a retained `frame_id` for later `compare_frames` calls. Pass `entity` to either `capture_stats` or `capture_screenshot` to isolate one entity subtree while retaining camera and lighting support.
- Temporary `position`/`look_at` overrides on `capture_stats` and `capture_screenshot` render the moved 3D camera before capturing its image target, then restore camera state. No extra caller delay is needed for objects outside the original view.
- `compare_frames` - Compare two retained captures and report pixel-difference magnitude, changed percentage, bounding box, and centroid.

## Debugging Geometry Problems

When geometry looks wrong (wrong size, shape, missing, or occluded), follow this diagnostic protocol in order. Do NOT skip to screenshots - spatial tools give definitive answers faster.

1. **`check_all_overlaps(ground_y=0)`** - run scene-wide first. Catches: interpenetrating entities, models sunken below ground, floating objects. If this returns problems, fix them before anything else. This is the single highest-value diagnostic call. For follow-up on one entity, use `check_overlaps(entity=...)`.
2. **`get_bounding_box`** on suspect entities - compare actual dimensions vs intended. A "table" with height 0.01 is a plane, not a table. A "wall" with equal X/Y/Z is a cube, not a wall. Mismatched dimensions are the #1 cause of "it doesn't look right."
3. **`query_spatial`** between entity pairs - check distance and direction. "The chair should face the desk" becomes: is the direction vector from chair to desk aligned with the chair's forward? Answers relative positioning questions without visual ambiguity. Use `query_spatial_neighborhood(entity=..., radius=...)` when you need every nearby entity.
4. **`capture_depth`** - when you suspect occlusion or visibility issues. Returns entity names at normalized sample points cast through the selected `Camera3d` projection. If you expect to see `lamp_1` at screen center but depth reports `wall_east`, the lamp is occluded. Diagnoses "wrong entity showing" without guessing from pixels.
5. **`capture_stats`** - quantify brightness, clipping, flat output, or a deterministic before/after change without transferring an image.
6. **`capture_screenshot`** - last, for visual polish only. Colors, lighting, bloom, material appearance. By this point, structural issues should already be resolved.

**Rule**: if you're about to take a second screenshot to debug the same geometry issue, stop. Use the structural and numeric tools first.

## Batched Schedules

- `schedule_actions` - Submit batched, timed tool calls that execute inside the engine frame loop. Same-`at` actions fire in the same frame (atomic). Supports time offsets (`at`), frame offsets (`at_frame`), `stop_on_error`, `skip_if_error`, and sync/async modes.
- `get_schedule_result` - Poll status of an async schedule.

**Use `schedule_actions` instead of sequential tool calls** when you need atomic multi-step operations (pause → seek → screenshot → resume), time-lapse captures, or any workflow where intermediate frames between tool calls would cause drift. See `guide://scene-editing` for examples.

## Hot Reload & Time Control

See `guide://hot-reload` for reload modes (Full vs Partial), type re-aliasing, memory profiling, plugin delta detection, and keyboard shortcuts (F5/F6/F7). See `guide://scene-editing` for time control commands (`pause_time`, `resume_time`, `set_time_scale`) and batched schedule workflows.

For Rust-owned applications loading and hot-reloading Python plugins, see
`guide://native-plugins`.
`PyBevyPlugin::with_plugin` supports build-time configuration; combining it with
native hot reload is currently rejected.

## Critical Rules

Integer rectangle arithmetic raises `OverflowError` for out-of-range results; `IRect` center constructors require non-negative sizes. `URect.inflate` saturates coordinates and rejects an unrepresentable negation.

Owned nested UI gradient values and `Isometry2d` vector fields are read-only snapshots; replace the whole parent field to change them. Live accumulated-mouse and `ComputedNode` vector fields write through only with mutable resource/component access and expire at system exit.

`MouseMotion.delta` and `CursorMoved.position`/`delta` are also read-only child snapshots; replace the complete field on an owned message before sending it.

- Primitive dimensions and their corresponding field alternative are mutually exclusive: use `Cuboid(x_length=..., y_length=..., z_length=...)` or `Cuboid(half_size=...)`, not both, even when dimensions equal their defaults.
- For these dual-form constructors, omit defaulted arguments instead of passing `None`.
- Supply both `minor_radius` and `major_radius` for Torus's field form, or both `cos` and `sin` for Rot2. Omit both Rot2 inputs for identity; explicit `None` is invalid.

- **Canonical entrypoint structure** - every scene file must follow this exact pattern:
  ```python
  from pybevy.prelude import *

  @entrypoint
  def main(app: App) -> App:
      return (
          app.add_plugins(DefaultPlugins)
          # ... add systems, resources, etc.
          .add_systems(Startup, setup)
          .add_systems(Update, animate)
      )

  if __name__ == "__main__":
      main().run()
  ```
  Common mistakes: missing `@entrypoint` decorator, wrong signature (must be `def main(app: App) -> App`), missing `return app`, missing `if __name__` guard.
- **Always use chained return style** - use `return (app.add_plugins(...).add_systems(...))`. Do NOT use separate `app.method(...)` calls followed by `return app`.
- Always use `from pybevy.prelude import *` (NOT `from pybevy import *`)
- Pass shapes directly to meshes.add(): `meshes.add(Cuboid(1,1,1))`
- Compare enum values with constructed variants, such as `MouseButton.Left()`. Comparisons with uncalled variant classes from the same family raise `TypeError`; compact constants such as `KeyCode.KeyA` keep their existing spelling.
- `AudioPlayer` accepts `AudioSource` and `Pitch` handles. Query procedural players with `AudioPlayer[Pitch]`; bare `AudioPlayer` queries select `AudioSource` players.
- Use `GlobalAmbientLight` (Resource) not `AmbientLight` (Component) for global light
- Changing `@component` or `@resource` field structure (add/remove fields, change storage mode) works with `reload` Full mode, but use `run_scene` if behavior is unexpected
- Start with `asset_server.load_image("texture.png")` or `asset_server.load_audio("sound.ogg")`. These are equivalent to `load(path, asset_type=Image)` and `load(path, asset_type=AudioSource)`; both styles preserve precise handle types. The general `load` requires a keyword-only type and supports other asset classes too. For image/glTF settings, use `asset_server.load_builder().with_settings(settings).load(path)`; settings establish the type. Builders expire with the system. `load_with_settings` is deprecated.
- For 3D models: `from pybevy.world_serialization import WorldAssetRoot, WorldAsset` then `asset_server.load("model.glb#Scene0", asset_type=WorldAsset)`. Spawn with `commands.spawn(WorldAssetRoot(handle), Transform.from_xyz(...))`. **GLB models often have origin at center** - a 1-unit-tall model spawned at Y=0 will be half-buried. Apply `y_offset = height / 2`. See `guide://3d-models`.
- **Do NOT use `Text2d` in 3D scenes.** `Text2d` requires `Camera2d` and will not render with `Camera3d`. For text overlays, HUDs, or labels in 3D scenes, use UI `Text` (from `pybevy.ui`) with a `Node` component. See `guide://ui-text`.

## JSON Mutation Formats

Custom component fields annotated with `Optional[Mode]` or `Mode | None`, where
`Mode` is a Python enum, accept a variant name or JSON null, including when the
current value is null. Invalid names report the available variants.

Python field reads use `{"serialization_error": ...}` markers for cycles and nesting beyond 64 levels. Dictionary key/value pair arrays are read-only.

Reflected struct enum variants must include every required field. Incomplete payloads return an error naming the missing field and leave the component unchanged. Reflected single-field tuple variants accept either their direct payload value or a one-element array. For a tuple variant whose single payload is a default-constructible wrapper, an object may provide only the fields to override; omitted fields use that wrapper's constructor defaults. Null payloads for named variants are rejected, including the current variant.

MCP mutation, schema, and query tools address types registered with the active
World. To create the first value of an otherwise unused custom class, call
`world.register_component(Type)` or `world.register_resource(Type)` in scene
code or `run_code`. A class defined inside `run_code` must be registered in
that same call. Decoration or reload alone does not register an unused type.

When using `set_component`, `spawn_entity`, or `set_resource`, field values are automatically converted:

- **Enum fields** (Color, PlaybackMode, etc.): `{"Srgba": {"red": 1.0, "green": 0.5, "blue": 0.0, "alpha": 1.0}}` or unit variant as string: `"Manual"`
- **Color shorthand**: `[1.0, 0.5, 0.0, 1.0]` (RGBA array, Python fallback path)
- **Vec2/Vec3/Vec4**: `[x, y]`, `[x, y, z]`, `[x, y, z, w]`
- **Option fields**: `null` for None, value directly for Some
- **Nested structs**: `{"x": 1.0, "y": 2.0, "z": 3.0}`
- **Resources**: `set_resource` patches existing fields - only provided fields are updated, others preserved. Native-resource patches validate all conversions and setters on a detached value; failure leaves the resource unchanged. Types that cannot safely stage a copy reject patches without mutation. Its response returns `resource` with the type name and `inserted: true` for a newly created value or `inserted: false` for a patch.

## Available Guides

Call `get_guide("index")` for the full list of available guides with descriptions, then call `get_guide(name)` for an individual guide (for example, `get_guide("patterns")`, `get_guide("lighting")`, or `get_guide("recipes/outdoor")`). Clients with MCP resource support may instead read the equivalent `guide://index` and `guide://{name}` resources. Generated resource aliases such as `get_guide_patterns` are optional and must not be assumed to exist. These URIs are never filesystem paths.

## Scene Generation (MANDATORY)

### Incremental Development (CRITICAL)
**Never write the entire scene in one shot.** Large scenes (500+ lines) will exceed the output token limit and fail. Instead:

1. **Start small** - Write a ~150-250 line initial scene with: camera, lighting, fog, ground, and the 2-3 most important entities. Load it with `run_scene`.
2. **Screenshot and verify** - Use `reload_and_capture` to confirm the foundation works and looks correct.
3. **Add detail iteratively** - Edit the file to add more entities, materials,
   and animations. The file watcher reloads saved Python changes; use an explicit
   Full reload only when a clean reset is needed.
4. **Build in layers** - Each iteration adds one category: first geometry, then materials/colors, then lighting refinement, then animation, then polish.

### Before Coding
1. **Call `get_guide("patterns")` + ONE matching recipe** if available. That's enough to start. Read topic guides (lighting, materials, etc.) later when iterating on those specific features. For scenes with GLB models, also call `get_guide("3d-models")`.
2. **Extract every noun** from the prompt → each becomes at least one entity
3. **Plan camera** → default to eye-level (Y=2–4), NOT overhead
4. **Plan 3 depth layers** → foreground framing, midground content, background/fog

### Post-Screenshot Self-Review
Before presenting to the user, verify: all geometry visible (no pure-black areas), 3 depth layers readable, every prompt noun has an entity, at least 2 distinct colors, camera at appropriate height. **If ANY check fails:** double ambient brightness, brighten materials +0.1, re-capture.

## Headless Rendering

For environments without a display server (CI, remote servers, containers), PyBevy supports headless GPU rendering:

Configure groups through their builders and apply them with `app.add_plugins(builder)`.
`PluginGroupBuilder.build()` takes no App argument and returns the same builder.
Custom groups implement `build(self) -> PluginGroupBuilder`; start their empty
builder with `PluginGroupBuilder.start(MyGroup)`. Public `finish(app)` applies
native groups or builders. Builders remain reusable
with other Apps; duplicate native plugins in the same App raise `RuntimeError`.

1. **Scene setup** - disable WinitPlugin, use `ScheduleRunnerPlugin`, and render to `RenderTarget.Image(ImageRenderTarget(handle=handle))`:
   ```python
   (app.add_plugins(
       DefaultPlugins()
       .set(WindowPlugin(primary_window=None, exit_condition=ExitCondition.DontExit))
       .disable(WinitPlugin)
   )
   .add_plugins(ScheduleRunnerPlugin.run_loop(16)))
   ```
   Camera must use an offscreen render target:
   ```python
   render_target = Image.new_render_target(width=256, height=256)
   handle = images.add(render_target)
   commands.spawn(Camera3d(), Camera(), RenderTarget.Image(ImageRenderTarget(handle=handle)), transform)
   ```

2. **Launch with MCP** - use `run_scene(path=..., headless=True)` to start the scene without a window backend.

3. **Capture tools work** - `capture_screenshot`, `capture_stats`, `capture_turnaround`, and `capture_timeline` all fall back to GPU readback when no window exists.

4. **Full setup** - read `guide://headless` for a complete working scene.

Texture flags belong to `pybevy.render.TextureUsages`; asset world flags belong to
`pybevy.assets.RenderAssetUsages`. Both support in-place `insert`, `remove`,
`toggle`, and `set`; flag constants return fresh values.

`Image.texture_descriptor.usage` and `CameraMainTextureUsages.value` borrow their parent's access
mode and lifetime. Nested writes persist through mutable asset/query access.
Owned parents return read-only snapshots; copy and replace the parent field.
Use `copy.copy()` for independent mutable flags. The same rules apply to
`Image.asset_usage`, `Mesh.asset_usage`, `ImageLoaderSettings.asset_usage`,
and `GltfLoaderSettings.load_meshes`/`load_materials`.

## Getting Started

When you call the `get_started` tool, pass `confirmation_key: "pybevy-ready"` to confirm you've read these instructions. This avoids receiving duplicate content.
