# Hot Reload Guide

Full vs partial reload, what persists across reloads, error recovery, and diagnostics.

## Reload Modes

### Full Reload
- Clears all entities and custom Python resources
- Built-in (engine-plugin) resources survive as resources but are reset to their
  engine defaults, including Time, GlobalVolume and WireframeConfig. The virtual
  Time clock therefore resets to 0
- `GizmoConfigStore` is the one resource preserved by value, because its contents
  are plugin-populated and cannot be rebuilt from defaults
- `AssetServer` is never reset. Assets whose last strong handle was dropped by the
  despawn are reclaimed by Bevy's asset GC, so their ids and generations change
- Re-runs ALL systems including Startup (re-creates the scene)
- Entity IDs will change - use `Name` component for stable references
- Custom `@component` and `@resource` types are re-aliased by name (no new ComponentId if structure unchanged)
- Observers are cleared and re-registered automatically (see Observers Across Reloads)
- Old DynamicSystems are cleaned up to prevent memory leaks

### Partial Reload
- Preserves all entities and resources (including custom ones)
- Preserves `Added`/`Changed` history for matching system registrations, including
  conditions and individual pipe stages. Reload alone does not make unchanged
  components appear new; changes since a skipped system last ran remain visible.
- Does NOT preserve `Local[T]`: reloading a system constructs its locals
  again, so keep surviving state in a `@resource`
- Only reloads Update/Last system functions
- Startup systems are NOT re-run
- Use for iterating on game logic without resetting scene state
- Auto-escalates to Full when Startup systems, observers, the set of resource
  types, or a custom `@component`/`@resource` field layout changed since the
  previous successful reload
- The baseline is seeded from the definitions the entrypoint built, so the first
  Partial reload after `run_scene` compares like any later one and keeps the
  state accumulated since launch

History matches the schedule, qualified callable names, pipe/condition structure,
ordering configuration, and occurrence within otherwise identical registrations.
Keep duplicate registrations in the same declaration order, or distinguish them
with separate system sets. New or renamed registrations, changed configuration or
declared access, and full reloads start with fresh history. Changing a function's
body alone preserves history. A rejected candidate does not consume the previous
generation's history. `Local[T]` and message-reader cursors are not transferred.

Partial mode does not reconstruct retained Startup-owned state. These
consequences matter during iteration:

- Changing annotated `@resource` fields escalates to Full and rebuilds the
  resource. Changing only a default value keeps the current instance on Partial;
  use Full to apply the new initial value.
- When the field layout is compatible, the preserved resource accepts nested
  class and enum values declared by the successfully loaded generation. Existing
  field objects are not migrated: an old enum value remains an old enum value
  until scene code assigns a current-generation replacement. Rejected reloads do
  not approve their candidate types for later assignment.
- Mesh assets and handles created by Startup survive a Partial reload. Editing
  mesh-building code has no visible effect until Startup runs again; use a Full
  reload when changing generated geometry.
- Module-level state does not survive a reload. Re-executing the scene module
  rebinds `history = []`, caches and accumulators to their initial value; a Full
  reload reruns the Startup system that filled them, a Partial reload does not.
  The app keeps running at full speed with `get_last_error` null and no log
  line, so the emptied value shows up only by reading the global. Own that state
  in a `@resource`, which a Partial reload preserves.

## MCP Reload Commands

```
reload {"mode": "full"}    - Full reload (default)
reload {"mode": "partial"} - Partial reload

# Atomic time control with reload (avoids timing drift between separate calls):
reload {"mode": "full", "pause": true}                - Reload and freeze immediately
reload {"mode": "full", "time_scale": 0.1}            - Reload at slow-motion
reload {"mode": "full", "pause": true, "time_scale": 0.1} - Both: frozen at 0.1x, resume when ready

get_reload_status                  - Check if reload is pending/complete
get_last_error                     - Get Python error traceback if reload failed
```

## Workflow: Edit and Reload

Scenes launched through `run_scene` are watched. Saving a Python file queues a
Partial reload by default, so do not call `reload` after every edit. Structural
changes may automatically escalate that request to Full.

1. Edit and save the Python source file
2. Check `get_reload_status` or call a capture tool to inspect the updated scene
3. If errors occur, use `get_last_error` for the Python traceback

Use `reload {"mode": "full"}` after saving when you specifically need a clean
reset. A file-watcher request may already have run by then; an explicit Partial
reload cannot restore state that an earlier Full reload cleared. The CLI's F6
shortcut changes the mode used by later file saves.

For CLI sessions, `pybevy dev scene.py --full` and `pybevy watch scene.py --full`
start the watcher in Full mode; without `--full`, both start in Partial mode.
This selects the initial mode only, so F6 still applies to later saves.

## Keyboard Shortcuts (CLI hot-reload mode)

| Key | Action |
|-----|--------|
| **F5** | Trigger Full reload |
| **F6** | Toggle between Full and Partial mode |
| **F7** | Toggle memory overlay (RSS, GC objects, per-reload deltas) |

## When to Use Each Mode

| Scenario | Mode | Why |
|----------|------|-----|
| Changed Startup system (scene setup) | Full | Need to re-run Startup to apply |
| Changed Update system logic | Partial | Faster, keeps scene state |
| Added new `@component`/`@resource` types | Full | Need fresh registration |
| Changed `@component` fields or storage mode | Full | Partial reload auto-escalates before applying the new layout |
| Changed `@resource` fields | Full | Partial reload keeps the live instance, so it auto-escalates to rebuild it |
| Tweaking animation parameters | Partial | Preserves entity state |
| First Partial reload after normal scene launch | Partial | Initial definitions provide the comparison baseline |
| Added/changed observers | Full | Auto-escalated from Partial |
| Renamed/removed a system function | `run_scene` | Stale schedule entries persist across `reload` |
| Edited a `.wgsl` shader or other asset | none | Bevy re-loads the asset itself; the Python watcher ignores non-`.py` files |
| Scene looks broken | Full | Clean slate |

Asset files are watched while hot reload is active, so editing a shader or texture
updates the running scene with no `reload` call. Asset paths resolve against the
directory the scene process was launched from, not the scene file's own directory,
so editing a same-named file next to the scene changes nothing.

The Python watcher uses the same project root as module flushing. For a scene
inside the launch directory that root is the launch directory; for an absolute
scene outside it, the root is the scene's directory. An edit reloads the scene
only when it changes the scene itself or a statically imported dependency.
Unrelated Python files under the root are ignored. Scenes that use dynamic
imports reload conservatively for Python edits under the root. Asset files such
as `.wgsl` remain owned by Bevy's asset watcher.

The first Partial reload after `run_scene` preserves live state when definitions
are unchanged. Escalation to Full discards live edits and changes entity IDs;
it is reported in reload status and logs. Address entities by `Name` when they
must survive a Full reload.

## Entity ID Stability

Entity IDs are NOT stable across full reloads. Always use `Name` components:

```python
# In Python source
commands.spawn(Transform(), PointLight(intensity=1000), Name("sun"))
```

```
# In MCP - address by name
set_component {"entity": "sun", "component": "PointLight", "fields": {"intensity": 2000}}
```

## Error Recovery

If a reload introduces a Python error:
1. Import and entrypoint failures keep the previous world running; rejected
   system registrations roll back the active generation
2. Call `get_last_error` to see the traceback
3. Fix the Python source
4. Call `reload {"mode": "full"}` to retry

Failed reloads restore the previous scene and helper module registrations, so
`run_code` can inspect the previous generation. This does not undo arbitrary
import-time side effects, such as file writes.

After successful recovery, the debug overlay clears the failure indicator and
hides the previous error. Full reload resets the scene clocks but does not stall
overlay or performance-statistics refreshes; errors in the new scene replace
the displayed error even when their timestamps restart at zero.

Runtime system failures are also printed to stderr once per registered system
generation; `get_last_error` continues to expose the latest failure. SSE events
(`/mcp/v1/sse`) broadcast errors in real-time.

## Type Re-aliasing (`@component` / `@resource`)

Custom Python types defined with `@component` or `@resource` survive hot reloads via **name-based re-aliasing**:

- After reload, Python classes get new `PyTypeObject` pointers
- The reload system matches new types to existing `ComponentId`s by qualified name (e.g., `__main__.Player`)
- If the structure matches (same storage mode, same fields), the existing `ComponentId` is reused - no data loss
- If the structure changes (e.g., added a field, switched storage mode), a fresh `ComponentId` is allocated

This means `reload` with Full mode handles most `@component`/`@resource` changes. Use `run_scene` only when you need a guaranteed clean-slate restart.

## Observers Across Reloads

Where an observer is registered decides how a reload treats it:

| Registration | In the reload fingerprint? | Full reload | Partial reload |
|--------------|---------------------------|-------------|----------------|
| `app.add_observer(fn)` in `@entrypoint` | Yes (escalates Partial to Full when added or changed) | Cleared, then re-registered from the new definitions | Re-registered from the successfully committed definitions |
| `world.add_observer(fn)` in a system | No | Cleared, then re-created when Startup re-runs | Kept as-is |
| `entity.observe(fn)` | No | Removed with its target entity, re-created when Startup re-spawns it | Kept as-is |

Runtime registrations exist only after a system has run, so definition loading
cannot see them and they never influence the reload mode. `get_reload_status`
therefore reports no observer escalation for them. Their real change signal is
the code of the Startup system that registers them: adding or removing a
`world.add_observer(...)` call edits that system and escalates to Full on its
own. Editing only the observer body is picked up without escalation, because
observer callables are re-resolved by module and function name each generation.

Full reload clears the whole Python observer registry before Startup re-runs, so
a `world.add_observer` registration is replaced rather than duplicated. Entity
observers are removed with their targets and must be registered again when
Startup creates the replacement entities.

Partial reload replaces only observers declared by `app.add_observer` in the
entrypoint. Their callable and custom-event class therefore follow the committed
scene generation without duplicate registrations. Observers registered later by
`world.add_observer` or `entity.observe` remain attached to their existing event
class and target, just as their runtime entities remain in the World.

## Plugin Delta Detection

When plugins are added or removed across reloads, the reload system detects the delta:

- **New plugins**: Reported in `get_reload_status` as `plugins_added`
- **Removed plugins**: Reported as `plugins_removed` (restart may be required)
- Core Bevy plugins (DefaultPlugins, etc.) cannot be hot-removed - a restart is needed

The delta compares with the last successfully committed generation, starting
with the initial App declarations, and is reported once. An unchanged reload
reports neither field, and a failed candidate does not publish or advance the
comparison baseline.

New bridge-backed Rust plugins cannot be installed into an App that has already
started. If `plugins_added` includes one, use `run_scene` before relying on its
resources or systems. For custom materials, install `ShaderMaterialPlugin()` on
the first run; new `@material` classes added later reuse that plugin.

## Plugin.build() During Reload

Custom Python plugins that define a `build()` method are re-executed during hot reload. Systems, resources, messages, and observers registered inside `build()` are captured in the temp app's pending collections and applied to the live app after reload completes.

```python
@plugin
class MyGamePlugin(Plugin):
    def build(self, app: App) -> None:
        app.insert_resource(GameState())       # Captured in pending_resources
        app.add_systems(Update, game_logic)    # Captured in pending_systems
        app.add_message(DamageEvent)           # Captured in pending_messages
```

**What is NOT re-executed during reload:**
- `DefaultPlugins` and other `PluginGroup` types - these persist from the initial load
- Bridge-backed plugins (built-in Rust plugins) - they call `with_bevy_app()` which temp apps lack
- New bridge-backed plugin types added mid-development require a `run_scene` restart

## Selective Module Flushing

Hot reload uses an AST-based import graph to minimize the modules flushed from `sys.modules`:

- On startup, all `.py` files under the watch root are parsed and import dependencies are tracked
- When files change, only the changed files and their transitive dependents are flushed
- This makes reloads faster for large projects (unchanged modules stay cached)
- F5 with Full mode always does a complete entity reset regardless of module flushing scope

The flush and the import graph share one root: the **launch directory** (the
directory the scene process was started from) when the scene file is inside
it, otherwise the **scene's own directory**. A nested scene (for example
`scenes/foo.py` started from the project root) therefore picks up edits to
project-root helper modules, while a scene launched by absolute path from
outside the launch directory uses its own directory to reload sibling helpers.

## Memory Profiling

Press **F7** to toggle the memory overlay, which shows:

- Current RSS (resident set size) and growth since baseline
- Python GC object count
- Schedule system count
- Per-reload memory deltas (rolling window of last 20 reloads)
- Warning indicator if RSS growth exceeds threshold (100 MB default)

Memory data is also available via MCP:
```
get_performance  - includes memory_growth_mb, memory_peak_mb, memory_warning, reload_memory_snapshots
```

`reload_memory_snapshots`, `total_schedule_systems`, `current_generation_systems`,
`python_gc_objects` and `last_reload_mode` are always present: before the first
reload they read `[]`, the live counts, and `null`. `memory_growth_mb`,
`memory_peak_mb` and `memory_warning` still appear only once there is something
to report. All megabyte figures in this response (`memory_mb`,
`total_memory_mb`, `memory_growth_mb`, `memory_peak_mb`, and the per-snapshot
`rss_mb`/`delta_mb`) are JSON numbers that can be compared or plotted directly.

The same response distinguishes process lifetime from reload lifetime:
`uptime_secs` remains monotonic across reloads, while
`generation_uptime_secs` resets after a full reload.

`memory_growth_mb` is RSS since the first stats tick (about a second after startup),
not reload leak: it reads hundreds of MB at `reload_count: 0`. For per-reload growth
use `delta_mb` in `reload_memory_snapshots`.

Retired views release their bin-unpacking bind groups and cached pipeline keys during render cleanup;
allocator and driver caching can still keep reported memory above its startup level.

## System Rename/Removal Detection

After a successful reload, removed or renamed callable names are logged and
reported by `get_reload_status` as `systems_removed`. Failed candidates do not
change this comparison baseline. The field is a distinct-name delta for that
successful reload, so registering the same callable more than once does
not repeat its name.

Bevy schedule nodes are retained for one rollback generation, then physically
compacted after `Last` when the next generation commits. This applies even when
the new generation registers no systems. Generation guards keep predecessor
nodes from executing. During that bounded window, `get_system_list()` reports:

- `active`: a current-generation node;
- `rollback_retained`: a predecessor occurrence with a matching current
  schedule-local name occurrence;
- `retired`: an unmatched predecessor occurrence, such as a removed or renamed
  system, waiting for compaction.

After compaction, retired entries no longer appear. Use `run_scene` when you
need an immediate fresh process rather than the bounded reload window. Pass
`include_internal=True` only when diagnosing the full Bevy scheduler, including
PyBevy and engine-internal systems. Resource clients can read
`scene://systems` or `scene://systems/all`, respectively.
State schedules use stable qualified names such as `OnEnter(game.Phase.READY)`,
`OnExit(game.Phase.READY)`, and
`OnTransition(game.Phase.READY -> game.Phase.RUNNING)`.

## Time Continuity

Full reload resets `time.elapsed_secs()` to 0 (matches the "fresh play" mental model). Pause state and `relative_speed` are preserved: if you `pause_time` or `set_time_scale(0.1)` and then `reload`, the post-reload world is still paused and still at 0.1x. The same applies to `reload(pause=true, time_scale=...)`, which now actually take effect across the reset.

Partial reload (when it doesn't escalate) preserves the virtual clock, entities,
and custom resources. A custom resource can therefore hold an accumulated clock
across partial reloads. Full reload clears that resource along with the entities;
state that must survive a full reload needs an external store and must be restored
by Startup.

`Local[T]` is the exception. Resource values live in the world, but a local
lives in the reloaded system itself, so both modes construct it again.
Accumulate in a `@resource`, not a `Local`.

## Troubleshooting

### "resource not found in world"

- **Startup crashed before `insert_resource()`** - If setup errors mid-execution (e.g., wrong attribute name), resources inserted after the error line are never created. Check `get_last_error` or `get_logs(errors_only=true)` for tracebacks. Fix the error and use `run_scene` (not `reload`) to get a clean start.
- **Missing `app.insert_resource()`** - If your Startup system takes `ResMut[MyResource]`, the resource must already exist. Add `app.insert_resource(MyResource())` in your `@entrypoint` before `.add_systems(Startup, setup)`.
- **Used `reload` after changing `@component` field structure** - Adding/removing fields or changing storage mode allocates a new `ComponentId`. Full reload handles this (entities are recreated), but if behavior is unexpected, use `run_scene` for a clean restart.

### Entity count after failed reload

The entity count is unreliable in both directions, so never use it to judge whether Startup succeeded:

| Reload failed while | Entities |
|---|---|
| importing the module | previous run's entities still there |
| resolving or registering systems | previous run's entities still there |
| running a Startup system | scene emptied - entities are cleared *before* Startup re-runs |

Always check `get_last_error` first. A full `run_scene` gives a guaranteed clean state.

### GLB textures not loaded after reload_and_capture

`reload_and_capture` waits for a post-reload render with no queued shader
pipelines before counting `delay_frames` (default 30). This also applies when
`delay_frames=0`, and works with paused time. It does not wait for asynchronous
asset loads: GLB textures and meshes may still arrive after that render. For
scenes with GLB models, use `delay_frames=90` or higher. If textures still appear
missing (e.g., `Image:4` instead of `Image:15`), use `run_scene` followed by
`capture_screenshot` with a higher delay.
