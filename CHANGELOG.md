# Changelog

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
