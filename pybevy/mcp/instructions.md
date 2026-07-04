# PyBevy MCP — Python bindings for the Bevy game engine

## Workflow

1. **Read guides iteratively, not all upfront.** Start with `guide://index` to see all available guides, then read `guide://patterns` + ONE relevant recipe (e.g. `recipes/game-logic`, `recipes/outdoor`). That's enough to write the initial scene. Then read additional topic guides (lighting, materials, shadows, etc.) as you add those features in later iterations. Max 2-3 guides before first `run_scene`.
2. **When suggesting scenes** — propose ideas that showcase PyBevy's documented features: emissive bloom, glass/transmission materials, volumetric fog, parent-child hierarchy, day/night cycles. Read `guide://index` to see what's available, then suggest scenes that use those features.
3. **Run scene before scene tools** — `run_scene` must be called before any scene tool (get_component_schema, capture_screenshot, query_entities, etc.). Scene tools require a running Bevy subprocess.
4. **Read topic guides before API lookups** — Before using `search_api` or `get_type_definition`, check `guide://index` for a relevant curated guide. Guides are faster and more reliable than raw API exploration. Only fall back to API lookups for specifics not covered in guides.
5. **Use API lookup tools for specifics** — If you know the class name, use `get_type_definition('ClassName')` directly for the full definition. If you don't know the name, use `search_api('keyword')` first, then `get_type_definition` on the results.
6. **Call `run_scene` ONCE** — after that, edit the .py file + `reload`. Do NOT call run_scene again unless switching to a different scene file.
7. **After reload** — call `get_logs(errors_only=true)` to check for errors.
8. **Scene style defaults (3D)** — new 3D scenes MUST include: Bloom on the camera, DistanceFog for atmospheric depth, a warm key directional light (shadows) + cool fill directional light (no shadows, ~40% intensity), ClearColor matching fog color, and lighting above the minimum floors (see Scene Generation below). For 2D scenes, see `guide://2d`. Start bright, dim later.
9. **Interior camera placement** — for enclosed scenes, use `get_bounding_box` on walls/objects to calculate safe debug camera positions instead of guessing coordinates.
10. **Use `guide://scene-quality` as a reference** — consult its lighting floors and color palette rules when refining visuals, not necessarily before writing the first line of code.
11. **After first load with GLB models** — run `check_overlaps(ground_y=0)` to detect sunken models in one pass, before any visual iteration.

## API Lookup Guide

| Need | Tool | Example |
|------|------|---------|
| Know the class name | `get_type_definition` | `get_type_definition("PointLight")` → full constructor + methods |
| Don't know the name | `search_api` → then `get_type_definition` | `search_api("fog")` → find `DistanceFog` → `get_type_definition("DistanceFog")` |
| Need JSON spawn format | `get_component_schema` | `get_component_schema("Transform")` → field names + defaults |
| Curated topic knowledge | `guide://` resources | `guide://lighting`, `guide://camera`, `guide://patterns` |

**IMPORTANT**: `search_api` returns brief matches. Always follow up with `get_type_definition` for the full class definition including constructor parameters.

## Key Concepts

- **Entities** have numeric IDs that may change across hot reloads. Prefer Name-based addressing when possible.
- **Components** are data attached to entities (Transform, PointLight, etc.)
- **Resources** are global singletons (Time, AssetServer, etc.)
- **Systems** are functions that run each frame, organized by Stage (Startup, Update, Last, etc.)
- **Coordinate system** — Bevy is right-handed, Y-up. Camera default forward is −Z. When a camera on the −Z side looks toward +Z, the X-axis appears mirrored on screen (world +X = screen left). Plan grid layouts accordingly.

## Spatial Intelligence Tools

- `query_spatial` — Pairwise: distance/direction/overlap between two entities. Neighborhood: find entities within radius.
- `check_overlaps` — Single entity or scene-wide AABB overlap detection + floating entity detection. Use `ground_y` parameter to detect models sunk below a ground plane (AABB overlap alone won't catch this).
- `reload_and_capture` — One round-trip: reload → error check → screenshot. Replaces 3 separate calls.
- `capture_turnaround` — Multi-viewpoint orbit capture composited into one contact sheet. Auto-fits to scene bounds.
- `capture_depth` — RGB screenshot + ray-AABB depth samples. Returns **entity names at each sample point** (semantic segmentation), making it the primary tool for diagnosing **occlusion, visibility, and "wrong entity showing"** problems. Use it before repeated screenshots when geometry appears wrong.

## Debugging Geometry Problems

When geometry looks wrong (wrong size, shape, missing, or occluded), follow this diagnostic protocol in order. Do NOT skip to screenshots — spatial tools give definitive answers faster.

1. **`check_overlaps(ground_y=0)`** — run scene-wide first. Catches: interpenetrating entities, models sunken below ground, floating objects. If this returns problems, fix them before anything else. This is the single highest-value diagnostic call.
2. **`get_bounding_box`** on suspect entities — compare actual dimensions vs intended. A "table" with height 0.01 is a plane, not a table. A "wall" with equal X/Y/Z is a cube, not a wall. Mismatched dimensions are the #1 cause of "it doesn't look right."
3. **`query_spatial`** between entity pairs — check distance and direction. "The chair should face the desk" becomes: is the direction vector from chair to desk aligned with the chair's forward? Answers relative positioning questions without visual ambiguity.
4. **`capture_depth`** — when you suspect occlusion or visibility issues. Returns entity names at screen-space sample points. If you expect to see `lamp_1` at screen center but depth reports `wall_east`, the lamp is occluded. Diagnoses "wrong entity showing" without guessing from pixels.
5. **`capture_screenshot`** — last, for visual polish only. Colors, lighting, bloom, material appearance. By this point, structural issues should already be resolved.

**Rule**: if you're about to take a second screenshot to debug the same geometry issue, stop. Use steps 1-4 instead.

## Batched Schedules

- `schedule_actions` — Submit batched, timed tool calls that execute inside the engine frame loop. Same-`at` actions fire in the same frame (atomic). Supports time offsets (`at`), frame offsets (`at_frame`), `stop_on_error`, `skip_if_error`, and sync/async modes.
- `get_schedule_result` — Poll status of an async schedule.

**Use `schedule_actions` instead of sequential tool calls** when you need atomic multi-step operations (pause → seek → screenshot → resume), time-lapse captures, or any workflow where intermediate frames between tool calls would cause drift. See `guide://scene-editing` for examples.

## Hot Reload & Time Control

See `guide://hot-reload` for reload modes (Full vs Partial), type re-aliasing, memory profiling, plugin delta detection, and keyboard shortcuts (F5/F6/F7). See `guide://scene-editing` for time control commands (`pause_time`, `resume_time`, `set_time_scale`) and batched schedule workflows.

## Critical Rules

- **Canonical entrypoint structure** — every scene file must follow this exact pattern:
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
- **Always use chained return style** — use `return (app.add_plugins(...).add_systems(...))`. Do NOT use separate `app.method(...)` calls followed by `return app`.
- Always use `from pybevy.prelude import *` (NOT `from pybevy import *`)
- Pass shapes directly to meshes.add(): `meshes.add(Cuboid(1,1,1))`
- Use `GlobalAmbientLight` (Resource) not `AmbientLight` (Component) for global light
- Changing `@component` or `@resource` field structure (add/remove fields, change storage mode) works with `reload` Full mode, but use `run_scene` if behavior is unexpected
- Use `asset_server.load_image("path")` for images and `asset_server.load_audio("path")` for audio. The generic `asset_server.load(path)` requires an explicit asset type argument: `asset_server.load("path", Mesh)`.
- For 3D models: `from pybevy.world_serialization import WorldAssetRoot, WorldAsset` then `asset_server.load("model.glb#Scene0", WorldAsset)`. Spawn with `commands.spawn(WorldAssetRoot(handle), Transform.from_xyz(...))`. **GLB models often have origin at center** — a 1-unit-tall model spawned at Y=0 will be half-buried. Apply `y_offset = height / 2`. See `guide://3d-models`.
- **Do NOT use `Text2d` in 3D scenes.** `Text2d` requires `Camera2d` and will not render with `Camera3d`. For text overlays, HUDs, or labels in 3D scenes, use UI `Text` (from `pybevy.ui`) with a `Node` component. See `guide://ui-text`.

## JSON Mutation Formats

When using `set_component`, `spawn_entity`, or `set_resource`, field values are automatically converted:

- **Enum fields** (Color, PlaybackMode, etc.): `{"Srgba": {"red": 1.0, "green": 0.5, "blue": 0.0, "alpha": 1.0}}` or unit variant as string: `"Manual"`
- **Color shorthand**: `[1.0, 0.5, 0.0, 1.0]` (RGBA array, Python fallback path)
- **Vec2/Vec3/Vec4**: `[x, y]`, `[x, y, z]`, `[x, y, z, w]`
- **Option fields**: `null` for None, value directly for Some
- **Nested structs**: `{"x": 1.0, "y": 2.0, "z": 3.0}`
- **Resources**: `set_resource` patches existing fields — only provided fields are updated, others preserved

## Available Guides

Read `guide://index` for the full list of available guides with descriptions. Read individual guides via `guide://{name}` (e.g., `guide://patterns`, `guide://lighting`, `guide://recipes/outdoor`).

## Scene Generation (MANDATORY)

### Incremental Development (CRITICAL)
**Never write the entire scene in one shot.** Large scenes (500+ lines) will exceed the output token limit and fail. Instead:

1. **Start small** — Write a ~150-250 line initial scene with: camera, lighting, fog, ground, and the 2-3 most important entities. Load it with `run_scene`.
2. **Screenshot and verify** — Use `reload_and_capture` to confirm the foundation works and looks correct.
3. **Add detail iteratively** — Edit the file to add more entities, materials, animations. Use `reload` after each batch of changes.
4. **Build in layers** — Each iteration adds one category: first geometry, then materials/colors, then lighting refinement, then animation, then polish.

### Before Coding
1. **Read `guide://patterns` + ONE matching recipe** if available. That's enough to start. Read topic guides (lighting, materials, etc.) later when iterating on those specific features. For scenes with GLB models, also read `guide://3d-models`.
2. **Extract every noun** from the prompt → each becomes at least one entity
3. **Plan camera** → default to eye-level (Y=2–4), NOT overhead
4. **Plan 3 depth layers** → foreground framing, midground content, background/fog

### Post-Screenshot Self-Review
Before presenting to the user, verify: all geometry visible (no pure-black areas), 3 depth layers readable, every prompt noun has an entity, at least 2 distinct colors, camera at appropriate height. **If ANY check fails:** double ambient brightness, brighten materials +0.1, re-capture.

## Headless Rendering

For environments without a display server (CI, remote servers, containers), PyBevy supports headless GPU rendering:

1. **Scene setup** — disable WinitPlugin, use `ScheduleRunnerPlugin`, and render to `RenderTarget.image()`:
   ```python
   app.add_plugins(
       DefaultPlugins()
       .set(WindowPlugin(primary_window=None, exit_condition=ExitCondition.DontExit))
       .disable(WinitPlugin)
   )
   .add_plugins(ScheduleRunnerPlugin.run_loop(16))
   ```
   Camera must use an offscreen render target:
   ```python
   render_target = Image.new_render_target(width=256, height=256)
   handle = images.add(render_target)
   commands.spawn(Camera3d(), Camera(), RenderTarget.image(handle), transform)
   ```

2. **Launch with MCP** — use `run_scene(path=..., headless=True)` to bypass the display check.

3. **Screenshots work** — `capture_screenshot`, `capture_turnaround`, and `capture_timeline` all fall back to GPU readback when no window exists.

4. **Reference example** — see `examples/misc/headless_render.py`.

## Getting Started

When you call the `get_started` tool, pass `confirmation_key: "pybevy-ready"` to confirm you've read these instructions. This avoids receiving duplicate content.

## MCP Pack Customization

This project has an editable MCP pack at `.pybevy/mcp/`.

- `pack.toml` — uncomment `[overrides.tools]` lines to patch tool descriptions
- `instructions.md` — edit these instructions (auto-injected into LLM context on connect)
- `prompts.md` — on-demand prompts (loaded via prompts/get)
- `guides/*.md` — add or override guides (`*.default.md` are read-only reference copies)

When a tool description is misleading or missing context for this project,
suggest editing `.pybevy/mcp/pack.toml` to the user.
