# Changelog

## Unreleased

### CPU DLPack array interop

On CPython, `pybevy.array.Array` now exports owned contiguous storage through
the standard DLPack protocol, and `pybevy.array.from_dlpack()` copies a
C-contiguous CPU tensor into bounded storage. A zero-copy consumer receives an
exclusive storage lease, so every PyBevy access raises until the consumer
releases its tensor. Views, borrowed engine storage, boolean storage, and
explicit `copy=True` exports are detached copies. RustPython deliberately does
not expose this CPython capsule boundary.

### Legacy `native_*` macros removed (breaking)

The `native_component`, `native_asset`, and `native_resource` proc macros
have been removed along with the `NativeComponent` trait. They were unused
internally; use `pycomponent`, `pyasset`, and `pyresource` instead.

### MCP Hub removed (breaking)

The unauthenticated local `pybevy hub` process broker and its automatic
fallback have been removed. MCP scene requests now launch their owned
`pybevy dev` subprocess directly on Linux, macOS, and Windows. In environments
without a graphical session, use `run_scene(..., headless=True)`. Scene crashes
are reported with their captured output and are no longer silently restarted.

### Render-resource types moved to `pybevy.render` (breaking)

`Extent3d`, `TextureDimension`, `TextureFormat`, and `VertexFormat` moved to
`pybevy.render`, alongside `TextureViewDimension`, matching Bevy's public
module. The legacy `pybevy.wgpu` module has been removed.

### Asset handle UUID terminology (breaking)

`Handle.weak_from_u128()` is now `Handle.uuid_from_u128()`, and
`Handle.is_weak()` is now `Handle.is_uuid()`. UUID-backed handle reprs now use
`Uuid(...)`. This matches Bevy 0.19's `Handle::Uuid` variant; Bevy no longer has
a general weak-handle variant.

### Asset IDs now match Bevy (breaking)

`Handle.id()`, `AssetEvent.id`, `Sprite.as_asset_id()`, and `Assets` iteration
now return `AssetId[A]` rather than integers or non-owning handles. `Assets`
lookup/removal methods and `AssetServer` state queries accept either
`Handle[A]` or `AssetId[A]`. This removes the synthetic UUID encoding that was
previously used for index-based asset IDs. Bevy's exact `AssetId.Index` and
`AssetId.Uuid` variants are available for matching and variant-local field
access.

## 0.3.0

### Bevy 0.19

PyBevy now targets Bevy 0.19 (upgraded from 0.18). Bevy's own 0.18 -> 0.19
migration notes apply to any native Rust code embedding PyBevy.

### Bounding-volume corner types now mirror Bevy (breaking)

`Aabb3d.min`, `Aabb3d.max`, `BoundingSphere.center` and `Isometry3d.translation`
now expose `Vec3A`, matching the Bevy field types they wrap. They previously
converted to `Vec3` on read and back on write.

The conversion hid a defect: because the returned `Vec3` was a converted copy
rather than a view of the field, a nested write such as `aabb.min.x = 5.0` was
silently discarded. These properties now behave like every other wrapper field.
A field reached from an ECS-borrowed instance persists writes; one reached from
an owned instance returns a read-only snapshot and raises rather than dropping
the write.

Migration: pass and compare `Vec3A` instead of `Vec3` when reading these four
properties. `Vec3A(x, y, z)` constructs one.

Parameters that Bevy declares as `impl Into<Vec3A>` accept either spelling, so a
value read back from one of these types can be passed straight into the next
call. That covers `Aabb3d(...)`, `Aabb3d.from_min_max`, `closest_point`, `grow`,
`shrink`, `scale_around_center`, `BoundingSphere(...)`, `Isometry3d(...)`,
`Isometry3d.from_translation`, and the isometry transform-point methods.

Whole-field assignment (`aabb.min = Vec3A(...)`) is the supported way to change
a corner; assigning through a component or asset that owns the bounding volume
continues to work unchanged.

### Nested field writes no longer silently vanish

`Rect`, `URect` and `BorderRect` stored their corner vectors as plain wrapper
values, so a getter returned a clone while a setter was also exposed, and
`rect.min.x = 5.0` mutated a throwaway. All three are now storage-backed, so
such a write either persists (borrowed instances) or raises
`RuntimeError` (owned instances) instead of being discarded. `Aabb2d.min`,
`Aabb2d.max`, `BoundingCircle.center` and `Affine2.translation` had the same
defect and are fixed the same way.

`PluginGroupBuilder` no longer drops retained additions. Python-defined plugins
added with `add()` execute after the native group, native PyBevy plugins use
Bevy's real `add_before`/`add_after` ordering, and `set()` now applies Audio,
Image, and TaskPool plugins instead of failing during group construction.
Relative placement of a Python-defined plugin now raises immediately because
its `build(app)` callback cannot safely re-enter an App borrowed by Bevy's
native group builder.

### Mesh vertex-attribute arrays: bounded Array API (breaking)

Mesh vertex-attribute access now returns PyBevy's portable **bounded**
`pybevy.array.Array` (the bounded surface, identical on CPython and RustPython)
instead of a real-NumPy view exposed through a context manager. Reads are
zero-copy on CPython and safe **without** a `with` block: an escaped or
post-system reference raises a clean error instead of reading freed mesh memory
(the old numpy view could dangle). For anyone who needs real NumPy, `.to_numpy()`
(CPython) and `.copy()` remain available on the returned array.

Migration:

- Read access no longer uses a `with` block; the accessor returns the array directly:
  - `with mesh.positions() as p: ...` -> `p = mesh.positions()`
  - same for `normals()`, `uvs()`, and `attribute(id)`
- Mutable access keeps the `with` block but now yields the in-place bounded array
  (writes land directly in the mesh; the array is closed on exit):
  - `with mesh.positions_mut() as p: ...` (call unchanged; `p` is now a bounded array)
  - same for `normals_mut()`, `uvs_mut()`, and `attribute_mut(id)`
  - the bounded array carries only the documented surface, so NumPy-only idioms such
    as `p[0] = [1.0, 2.0, 3.0]` (list RHS) are not supported; assign elementwise
    (`p[0, 0] = 1.0`) or with a matching-shape array
- Removed the transitional `positions_array()`/`normals_array()`/`uvs_array()`/
  `attribute_array()` and their `*_mut_array()` variants: the base accessor names
  now return the bounded array
- Removed `positions_copy()` / `normals_copy()`. For a detached snapshot, call
  `.copy()` (bounded) or `.to_numpy()` (real NumPy, CPython only) on the read array:
  - `mesh.positions_copy()` -> `mesh.positions().copy()` (or `.to_numpy()`)
- Removed the `MeshAttributeContext` / `MeshAttributeContextMut` classes

Whole-array `Array.sum()` and `Array.mean()` now use a fixed eight-lane f64
accumulation order across backends, including for float32 inputs. This pins
deterministic results; low-order bits may differ from NumPy's reduction order.
Contiguous and borrowed float arrays use the tiled reduction path, while
non-contiguous arrays materialize in logical row-major order before applying
the same accumulation contract.

Writable contiguous arrays now expose `Array.lens()` for fused in-place
expressions. Numeric subscripts select final-axis lanes, with negative indices
supported; scalar and one-dimensional arrays use lane zero. The generic array
does not assign vector or color names based only on shape. Mutable mesh
attribute borrows use the same lens API inside their existing `with` scope.

Float32 element-wise array expressions now retain float32 intermediates on both
the contiguous tiled path and the dense fallback. Constants and scalar
broadcasts narrow once to float32; float64 array expressions continue to use
float64 intermediates.

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
- Removed the misleading `pybevy.input.Axis` resource facade. Bevy 0.19 keeps analog state on each `Gamepad` component; query `Gamepad` and use `get_axis()`, `get_axis_unclamped()`, `left_stick()`, or `right_stick()`.
- `Val` equality now mirrors Bevy exactly: payload comparisons are exact and all zero-valued units compare equal. `Val` is now unhashable, matching Bevy's lack of a `Hash` implementation, and a `Val.px(...)` no longer compares equal to a bare Python number on RP2.
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
- `Resource` now inherits `Component`, matching Bevy 0.19. Bridged native resources can be read through `Query[T]` on their stable resource entity and mutable wrappers through `Query[Mut[T]]`; `Res`/`ResMut` remain the preferred singleton system parameters and conflict with resource queries over the same value.
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
- Bevy 0.19 lighting features: `RectLight` area light (with the engine's `area_light_luts` feature enabled so it renders), screen-space `ContactShadows` camera component + `contact_shadows_enabled` flag on the three light types, and `ParallaxCorrection` for reflection probes; lighting/shadows guides cover all three
- Bevy 0.19 text features: `EditableText` input fields (engine-side typing/cursor/selection/clipboard, click to focus, poll `.value`) and the `LetterSpacing` component (`Px`/`Rem`); covered in the ui-text guide

### Bug fixes

- `Quat(x, y, z, w)` now raises a TypeError pointing at `Quat.from_xyzw`/`from_axis_angle`/`from_euler`/`Quat.IDENTITY` (was PyO3's bare "cannot create instances")
- Guides/docs: `Atmosphere` snippets now include the required `AtmosphereSettings()` camera component (not auto-inserted; the sky is silently absent without it); the lighting guide covers `Exposure` for physical sun illuminance and the one-sun rule for Atmosphere scenes; `guide://patterns` puts `DistanceFog` on the camera (it did nothing as a standalone entity); `SunDisk.EARTH` and `Exposure.SUNLIGHT`/`INDOOR`/`BLENDER` docstring examples use the bare class attributes (the called form raises `TypeError`)
- Dev loader: scenes now execute under their real module name and are registered in `sys.modules` (runpy named plain scripts `<run_path>` and left them unregistered). `import <scene>` from `run_code` resolves to the live scene classes instead of silently duplicating every class (registries are identity-keyed, so duplicates failed with confusing "expected X, got X" errors), and numba `cache=True` kernels defined in scene files survive relaunch (the cache pickled the unimportable module name). A scene file named like an already-imported module runs unregistered with a warning.
- MCP: `capture_timeline` no longer burns the hot-reload debug overlay into contact-sheet frames. Overlay hiding is refcounted (`OverlaySuppression`): screenshots, timelines, and turnarounds take a hold and the overlay's render system drives visibility from it each frame, so overlapping captures compose, the error text cannot pop back mid-capture, and a Full reload respawning the overlay mid-timeline no longer strands it. `capture_turnaround` now suppresses the overlay even with `hide_ui=false`
- Debug overlay entity count now matches `get_performance`/`list_entities` (scene entities; the raw allocator count also included resource-entities and engine internals, disagreeing by hundreds)
- Hot reload: Full reload no longer despawns the `Window`/`Monitor` entities winit creates at event-loop start. Bevy 0.19 links windows to monitors with a despawn-cascading relationship, so sweeping the post-baseline monitor entity killed the primary window and exited the app cleanly on the first reload. Engine entities alive before the first user Startup system are now also folded into the reload baseline.
- Hot reload: Partial reloads no longer escalate to Full on every save. Escalation now compares Startup-system and observer code hashes plus resource types against the previous generation, instead of escalating whenever the scene has any Startup system. Editing a Startup system body still escalates (the new code must re-run); edits that only change module-level globals still need a manual Full reload (F5).
- Hot reload: cleanup log counts no longer mix filtered and unfiltered totals ("299 live" vs "24 remaining"); both sides now count scene entities, with resource-entities reported separately.

### Performance

- The declared-access / world-cell change that makes system parameters sound under multithreaded scheduling (queries and resources now read through an `UnsafeWorldCell` bounded by the access declared at `initialize`, instead of a shared `&mut World`) carries a one-sided cost on the query hot path: query-heavy operations run roughly 10-13% slower and observer dispatch around 30% slower (measured; the observer figure is noisy), while View iteration is faster thanks to the now-cached `QueryState`. The query wrapper's Python entry points (`__next__`, `len()`, `single()`, `is_empty()`, `get()`, `iter_many()`) now perform the same `ValidityFlag` check that resource and component parameters already did, so a query iterator leaked past its system raises a clean error instead of advancing against a stale world cell; this adds one atomic load per query operation on top of the figures above.

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
