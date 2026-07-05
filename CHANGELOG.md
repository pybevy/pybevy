# Changelog

## Unreleased

### Bevy 0.19 upgrade

PyBevy now tracks Bevy 0.19.0 (from 0.18). PyBevy mirrors Bevy's API surface,
so upstream renames, module moves, and default changes flow through to Python.

#### Module moves (import paths changed, class names unchanged)

- `HalfSpace`: `pybevy.camera` -> `pybevy.math` (Bevy moved it to `bevy_math`)
- `Atmosphere`, `ScatteringMedium`, `Falloff`, `Skybox`: `pybevy.pbr`/`pybevy.camera` -> `pybevy.light` (Bevy moved them to `bevy_light`; `AtmosphereSettings` and `AtmosphereMode` stay in `pybevy.pbr`)
- `ScreenSpaceTransmissionQuality`: `pybevy.camera` -> `pybevy.pbr`
- New `pybevy.material` module mirroring Bevy's new `bevy_material` crate: `AlphaMode` (from `pybevy.render`) and `OpaqueRendererMethod` (from `pybevy.pbr`) moved there; `DefaultOpaqueRendererMethod` stays in `pybevy.pbr`
- `UvChannel`: `pybevy.pbr` -> `pybevy.mesh`

#### Renames and removals (mirroring Bevy 0.19)

- Module `pybevy.scene` -> `pybevy.world_serialization` (Bevy renamed `bevy_scene` to `bevy_world_serialization`; the "Scene" name was reused upstream for the new BSN system, which PyBevy does not adopt)
- `Scene` -> `WorldAsset`; `DynamicScene` -> `DynamicWorld`; `SceneRoot` -> `WorldAssetRoot`; `DynamicSceneRoot` -> `DynamicWorldRoot`; `SceneSpawner` -> `WorldInstanceSpawner`; `SceneInstanceReady` -> `WorldInstanceReady`; `ScenePlugin` -> `WorldSerializationPlugin` (`InstanceId` is unchanged)
- `AssetServer.load_scene()` -> `AssetServer.load_world_asset()`
- Lifecycle marker `Replace` renamed to `Discard` (`On[Discard, T]`)
- Lights: `shadows_enabled` -> `shadow_maps_enabled` (PointLight/SpotLight/DirectionalLight constructor kwarg, property, `from_numpy` kwarg, View column)
- `Atmosphere`: `bottom_radius`/`top_radius` -> `inner_radius`/`outer_radius`; `earthlike()` -> `earth()` (also on `ScatteringMedium`); `AtmosphereSettings.scene_units_to_m` removed
- `Camera3d`: `screen_space_specular_transmission_steps`/`_quality` removed; use the new `ScreenSpaceTransmission` component (`pybevy.pbr`) instead
- `ScreenSpaceReflections`: `perceptual_roughness_threshold` -> `min_perceptual_roughness`/`max_perceptual_roughness` range tuples; new `edge_fadeout`
- `OverflowClipBox` -> `VisualBox`; `ImageNode.texture` -> `ImageNode.image`; `Plane3dMeshBuilder` -> `PlaneMeshBuilder`
- `ComputedNode.stack_index` removed; query the new `ComputedStackIndex` component instead
- `FontAtlas.texture_atlas` now returns a `TextureAtlasLayout` (Bevy stores the layout inline; it is no longer a `Handle`); `FontAtlasKey` gained `id`/`index`/`variations_hash`
- `FontStyle` is now a proper enum: `FontStyle.Normal()`, `FontStyle.Italic()`, `FontStyle.Oblique(angle)` (was `NORMAL`/`ITALIC` constants + `oblique()`)
- `Cone.from_dimensions`/`Cylinder.from_dimensions` removed; `Cuboid.from_corners(min=, max=)` -> `(point1=, point2=)`
- `GltfAssetLabel.MorphTarget` removed (the sub-asset no longer exists in Bevy); `GltfAssetLabel.Material(n)` now labels a `GltfMaterial` sub-asset, the processed `StandardMaterial` lives under the `"/std"` label suffix
- `Shader.with_import_path()` removed (Bevy has no such builder); assign the `import_path` property instead
- `Frustum.from_clip_from_world`/`from_clip_from_world_custom_far` moved to the new `ViewFrustum` class (`pybevy.math`), mirroring Bevy 0.19's split where `Frustum` is a newtype over `bevy_math`'s `ViewFrustum`: build with `Frustum(ViewFrustum.from_clip_from_world(mat))`; the payload is exposed as `Frustum.value`
- `FontAtlasSet.get_by_font(font)` -> `items()` (Bevy 0.19 keys atlases by `FontAtlasKey`, not font handle; the old method had stopped filtering); the set is also iterable like a dict (`for key in font_atlas_set`, `len(font_atlas_set)`)
- `TextFont.font_size` now returns a `FontSize` enum value (`FontSize.Px(20.0)`, `.Vw`, `.Vh`, `.VMin`, `.VMax`, `.Rem`) mirroring Bevy's `FontSize`; plain floats are still accepted everywhere a size is written and mean `Px`. `TextFont.font` now returns a `FontSource` (`FontSource.Handle(h)`, `.Family(name)`, or a generic family like `.Monospace()`) and accepts a `FontSource`, `Handle`, or family-name `str`
- `TextFont.from_numpy` removed (Bevy's `FontSize` enum is not zero-copy batchable)
- `PerspectiveProjection(fov=None, ...)` no longer accepts `None`; parameters are plain floats

#### Behavior changes (same code, different result)

- `Ellipse(half_size)` now sets the half-size directly, matching Bevy's `Ellipse::new` (previously the argument was halved); `Ellipse()` defaults to half_size (1.0, 0.5)
- `Skybox`: `image` is now optional (`Skybox()` == Bevy's default with no image); the `image` property returns `None` when unset
- Constructor defaults aligned with Bevy 0.19 `Default` impls: `Cone`/`Cylinder` radius 0.5; `CircularSector`/`CircularSegment` radius 0.5, half_angle 2*pi/3; `RegularPolygon` circumradius 0.5; `Segment2d` endpoints (-0.5,0)/(0.5,0); `FocusPolicy()` is `Pass`; `TextShadow` offset (4,4), color rgba(0,0,0,0.75); `DistanceFog.directional_light_color` `NONE`; `ColorMaterial.alpha_mode` `Blend`; `Outline` width `px(1)`, color `WHITE`; `ShadowStyle` percent offsets/blur, color `BLACK`; `ColorStop`/`AngularColorStop` color `WHITE`; `ScreenSpaceAmbientOcclusion.constant_object_thickness` 0.25; `ScreenSpaceReflections` linear_steps 10, bisection_steps 5
- Bare `Query[Entity]` also matches resource entities in Bevy 0.19 (resources are stored as entities); filter with component markers where exact counts matter
- `PbrPlugin` now requires `GltfPlugin` to be added before it (Bevy's `PbrPlugin::build` registers a glTF material extension handler; `DefaultPlugins` order provides this automatically, manual plugin lists must add `GltfPlugin` first)
- System-parameter validation failures and command errors now panic via Bevy's default error handler (Bevy 0.19 validates parameters while fetching data and routes failures to the app error handler; on 0.18 such systems were skipped silently). Consequence for manual plugin lists: `LightPlugin` needs `ImagePlugin` and `GizmoPlugin` present or its atmosphere/light-gizmo systems panic (`DefaultPlugins` provides both automatically)
- `Image(...)` constructor now mirrors Bevy's `Image::new` parameter order (`size`, `dimension`, `data`, `format`, `asset_usage`): `data` moved from 2nd to 3rd position, so pass it as a keyword; a data/size/format length mismatch now raises `ValueError` instead of a debug panic

#### Additions

- `ScreenSpaceTransmission` component (`pybevy.pbr`): per-camera transmission `steps`/`quality`
- `ScatteringTerm` + `PhaseFunction` (`pybevy.light`); `ScatteringMedium(falloff_resolution=, phase_resolution=, terms=[...])` mirroring Bevy's `new`, plus `label`/`terms` properties
- `WireframeTopology` enum; `WireframeConfig.default_line_width`/`default_topology`; `WireframeMaterial.line_width`/`topology`
- `ComputedStackIndex` component (`pybevy.ui`): UI draw order, auto-added to nodes
- `Justify.Start`/`Justify.End`; `Tonemapping.BLENDER_FILMIC`; `Bloom.OLD_SCHOOL`/`SCREEN_BLUR`; `SunDisk.OFF`
- `FontSize` and `FontSource` enums (`pybevy.text`); `TextFont.with_family(name)`
- `FontWidth`/`FontStyle`/`FontHinting` types; `FontWidth(value)` constructor + `value` getter; `TextFont(width=, style=)`; `Node.direction` (`InlineDirection`); `ImageNode.visual_box`; `ImageArrayLayout.grid_count`/`grid_size`; `Atmosphere.mars()`/`ScatteringMedium.mars()`; `LightProbe.falloff`
- `Image(dimension=, format=, asset_usage=)` constructor params (mirroring `Image::new`); `RenderAssetUsages` flags mirroring Bevy's bitflags: `MAIN_WORLD`/`RENDER_WORLD` constants, `|`, `contains()`, `==`; `RenderAssetUsages()` is Bevy's default (both worlds)
- `TextureFormat` and `TextureDimension` now support value equality (`==` compared by identity before)
- `pybevy.gizmos` module with `GizmoPlugin`, mirroring `bevy_gizmos::GizmoPlugin` (initializes gizmo assets/config; required alongside `LightPlugin` in manual plugin lists, add it after `AssetPlugin` and `MeshPlugin`)
- `WireframePlugin` (`pybevy.pbr`): registers Bevy's wireframe render systems; previously the `Wireframe`/`WireframeConfig` wrappers existed but nothing rendered them
- `ScatteringMedium.terms` is now writable (mirrors the public field; e.g. appending a haze term to the earth defaults)
- New example `examples/bevy/large_scenes/bevy_city.py`: port of Bevy 0.19's procedurally generated city stress test (auto-downloads the CC0 Kenney kits on first run)

### Bug fixes

- `Quat(x, y, z, w)` now raises a TypeError pointing at `Quat.from_xyzw`/`from_axis_angle`/`from_euler`/`Quat.IDENTITY` (was PyO3's bare "cannot create instances")
- Guides/docs: `Atmosphere` snippets now include the required `AtmosphereSettings()` camera component (not auto-inserted; the sky is silently absent without it); the lighting guide covers `Exposure` for physical sun illuminance and the one-sun rule for Atmosphere scenes; `guide://patterns` puts `DistanceFog` on the camera (it did nothing as a standalone entity); `SunDisk.EARTH` and `Exposure.SUNLIGHT`/`INDOOR`/`BLENDER` docstring examples use the bare class attributes (the called form raises `TypeError`)

## 0.2.1

### Bug fixes

- Fix wgpu shader errors crashing the process — errors are now handled gracefully (#27, #9)
- Fix `StandardMaterial` import path (#26)
- Fix `DefaultPlugins` window title defaulting to "PyBevy App" (#25)
- Fix native-plugin feature and crate metadata for publishing

### Improvements

- Chained app style matching Bevy's `App::new().add_plugins(...)` pattern (#32)
- Macros split from monolithic file into type-specific modules (#28)
- Asset storage module restructured (#31)
- Vec3 format refactored (#24, #23)
- Code cleanups: unsafe documentation, top-level imports, comments (#29)
- Examples updated to use new chained app style

## 0.2.0

### Highlights

- **`@material` decorator** — Define custom shader materials in Python with texture slots, shader defines, and trait overrides. WGSL bindings are injected in-memory (no temp files).
- **`@entrypoint` decorator** — Cleaner app entry points with automatic `App` injection; required for `App.run()`.
- **`Optional[T]` queries** — Query components that may or may not be present on an entity, matching Bevy's `Option<&T>` syntax.
- **Generic `from_numpy()` batch spawning** — Batch-spawn any component (custom or built-in) from NumPy arrays via a unified `spawn_batch` API with zero-copy NumPy slice borrowing.
- **MCP spatial intelligence tools** — `query_spatial`, `check_overlaps`, `reload_and_capture`, `capture_turnaround`, and `capture_depth` for AI-assisted scene building.
- **Headless rendering** — Full GPU rendering without a display server, including screenshots, turnarounds, timelines, and depth captures. Enables CI/server-side rendering.
- **MCP Streamable HTTP transport** — Remote agent connections via the MCP Streamable HTTP spec, plus a PyBevy Hub process manager for headless environments.
- **JAX integration** — `Vec3`/`Quat` `from_jax()` write-back for View API, JAX array interop for batch component data.
- **`@component` storage enforcement** — `@component` now requires explicit `storage=` selection and errors on data fields without `@dataclass`, preventing silent bugs.

### Improvements

- **`--resolution` flag for `pybevy dev/watch`** — Set custom window resolution (e.g., `pybevy dev --resolution 1920x1080`) for dev and watch commands, with hot reload support.
- **View API** — Vec3/Vec2 compound field support in custom wrapper-storage components; bool and Color batch/View support; element-wise arithmetic and math methods on `ViewColumn`; `Entity` parameter support via `batch.entities()`; `view_only_fields` for exposing extra computed columns.
- **Hot reload** — Graceful failure handling with transitive dependency reloading; import graph tracking; plugin delta detection; observer and message type re-registration; system rename/removal detection; Python error overlay; frame-based F5 cooldown to prevent render pipeline corruption.
- **MCP server** — Session recording (`--record`); `schedule_actions` for batched frame-precise execution; `get_started` gate tool; field descriptions from docstrings in tool schemas; Bevy engine error surfacing after reload; guide-based iterative reading workflow.
- **Query performance** — Cache storage type alongside layout in query iteration (3.1x speedup); cached query layouts.
- **Bytecode VM** — Widened from f32 to f64 internal precision; modulo operator (`%`); fixed equality and sign bugs.
- **ECS** — `DespawnOnExit`/`DespawnOnEnter` systems; `Single[T]` deref support; nested Query iteration detection (matches Bevy borrow rules); `PartialEq`/`eq` added to 32+ types; `__repr__` for `Color`, `LinearRgba`, `Srgba`; `__copy__`/`__deepcopy__` for all component storage types; state types and `@plugin` added to prelude; `load_audio` convenience method on `AssetServer`.
- **Error handling** — System exceptions now propagate when hot reload is not active; improved numeric coercion error messages; structured errors for `set_resource`.
- **Stubs & types** — 48 missing method stubs added to `math.pyi` and `mesh.pyi`; stubs for `ChildOf`, `Children`, `MessageId`, `BorderRadius` builders, `GridPlacement` setters; `FontFeatureTag`, `AnimatedBy`, `Lightmap`, `ForwardDecal`, `FogVolume`, `Timer.almost_finish`, `FontWeight.clamp`, and more.
- **Modules reorganized** — `StandardMaterial` and PBR types moved to `pybevy_pbr`; `SkinnedMesh`, `ColorGrading`, `ScalingMode` moved to correct modules; `pybevy_storage` extracted as independent crate; `pybevy_shader` / `pybevy_shader_types` / `pybevy_wgpu` crates extracted.

### Bug fixes

- Fix hot reload deadlock: two-phase GIL acquisition in `control_poll` and lock ordering.
- Fix `MessageReader` delivering messages twice across frames.
- Fix `MessageWriter.write_default()` for custom Python messages.
- Fix message types not surviving hot reload.
- Fix GLB assets disappearing on F5 full reload.
- Fix `RenderLayers` introspection returning empty fields.
- Fix `run_system_once` queries seeing zero entities and incorrect generation.
- Fix `View` `RefCell` panic with `Changed`/`Added` filters.
- Fix `spawn_batch` inserting raw pointer bytes for wrapper-storage custom components.
- Fix cross-component View expressions not triggering change detection.
- Fix custom component re-add after removal and string-to-enum Python fallback.
- Fix `AnimationPlayer` ECS mutation persistence.
- Fix transform propagation to include parent-child hierarchy.
- Fix `iter_batches` to discover all matching archetype tables.
- Fix `Option<T>` field mutation and Vec/List field mutation.
- Fix MCP mode freeze when Bevy window is unfocused.
- Fix entity-specific `check_overlaps` sibling filtering for deep hierarchies.
- Fix `AssetEvent` nil UUID; migrate `LoadState` to PyO3 enum.
- Fix owned component field access panic with `OwnedReadOnly` snapshots.
- Fix `@material` types conflicting when used together in one system.
- Fix `capture_depth` image extraction using wrong key.
- Fix `ViewColumn` out-of-bounds read.
- Fix hot reload memory growth: clear param cache on failure paths.
- Fix temp file leaks: add cleanup for `delete=False` temp files.
- Fix `entrypoint` methods crashing during hot reload.
- Fix `Node.position_type` to accept `PositionType` enum.
- Fix GLTF component metadata: add setters and property decorators.
- Fix `emissive` setter to accept `Color | LinearRgba`.

### Breaking changes

- `@component` now requires explicit `storage=` parameter (no implicit default).
- `@entrypoint` decorator required for `App.run()`.
- Bare schedule names (`Startup`, `Update`, `Last`) used instead of `Stage.X`.
- `pybevy_mcp` crate renamed to `pybevy_control`.
- `render_readback` moved to `pybevy/_internal` (no longer public API).

## 0.1.0 — Initial Release

First public release of pybevy, a Python binding for the Bevy game engine.

### Highlights

- **Bevy 0.18** compatibility
- **Python 3.12+** support, including free-threaded Python (3.13t/3.14t)
- **Hot reload** — edit Python code and see changes instantly in running applications
- **ECS** — Query, View, Commands, Components, Resources, Observers, Messages, State
- **Rendering** — Camera2d/3d, PBR materials, lights (point/directional/spot), sprites, text, UI
- **Assets** — Mesh, StandardMaterial, ColorMaterial, Image, GLTF, Audio, Scenes
- **Input** — Keyboard, mouse, gamepad, touch
- **Animation** — Clips, graphs, players
- **Math** — Vec2/3/4, Quat, Mat3/4, geometric primitives with Meshable support
- **View API** — High-performance batch operations (30-50x speedup over Query), with optional Numba JIT and JAX integration
- **Custom components** — Define components in Python with `@component` decorator, full Query/View support
- **Native plugin** — Embed Python systems in Rust Bevy apps via `PyBevyPlugin`
- **MCP server** — AI agent integration for code generation assistance
- **Pre-built wheels** for Linux (x86_64), macOS (ARM + x86_64), Windows (x86_64)
