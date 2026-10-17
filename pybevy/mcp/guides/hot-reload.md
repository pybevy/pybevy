# Hot Reload Guide

Full vs partial reload, what persists across reloads, error recovery, and diagnostics.

Explicitly adding `HotReloadPlugin()` uses the App's reload state. A loader
attached afterward enables that same state, so watcher requests and
`HotReloadControl` requests advance the same generation counter.

The file watcher ignores build/cache directories by exact directory name below
the watch root, including `target`, `__pycache__`, and `node_modules`, every
directory whose name starts with `.` (`.git`, `.venv`, `.cache`, nested
`.worktrees`), plus any Python environment (a directory containing `pyvenv.cfg`
or `conda-meta/`, such as a project-local `venv/`). The watch root itself is
watched even when its name starts with `.`. Keep reloadable helpers out of
dot-directories; they cannot be imported as packages anyway. Project and helper
names that merely contain the ignored words are watched. Reload cleanup similarly
protects actual installed package directories, not project paths that happen to
contain `site-packages` or `dist-packages`.
The native-plugin loader uses the same boundary for PyBevy's module name, so a
user helper named `pybevy_helpers` is not protected from reload.

## Reload Modes

Changes saved during an active reload queue another attempt. The queued request
does not change the active attempt's import mode, and remains pending if that
attempt fails.

When a scene imports another module's `@entrypoint` and defines its own, the
launchers select the scene's own entrypoint. An imported entrypoint is used only
if the scene defines none.

Custom launchers using `load_entrypoint_function` can load a scene inside a
Python package that uses relative imports such as `from .helper import setup`;
the scene runs under its package module name. `project_dir` is both the
module-flush root and an import root, so a scene in a subdirectory can import
helpers from the project root.

### Full Reload
- Clears all entities and custom Python resources
- Built-in (engine-plugin) resources survive as resources but are reset to their
  engine defaults, including Time, GlobalVolume and WireframeConfig. The virtual
  Time clock therefore resets to 0
- Native resources first created by scene code are removed, including values
  inserted in the initial entrypoint or Startup. Plugin defaults are captured
  before user startup; later overrides do not turn them into user-only resources
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
- Reloads frame-system functions without re-running startup schedules
- Startup systems are NOT re-run
- Use for iterating on game logic without resetting scene state
- Auto-escalates to Full when Startup systems, observers, the set of resource
  types, a custom `@component`/`@resource` field layout, or the active or
  pending `@state` member changed since the previous successful reload.
  Only a Startup system's own code counts: editing a module constant or helper
  function it reads stays Partial and does not rerun it, so call
  `reload {"mode": "full"}` after changing such data
  Component layout includes private (`_`-prefixed) stored fields
- Startup condition changes include predicate code and AND/OR/NOT structure,
  for both fluent `run_if` configurations and `ConditionalSystem` expressions
- The baseline is seeded from the definitions the entrypoint built, so the first
  Partial reload after `run_scene` compares like any later one and keeps the
  state accumulated since launch
- Flushed project helper modules are reimported from source, including when an
  equal-size edit shares the previous import's timestamp second

History matches the schedule, qualified callable names, pipe/condition structure,
ordering configuration, and occurrence within otherwise identical registrations.
Keep duplicate registrations in the same declaration order, or distinguish them
with separate system sets. New or renamed registrations, changed configuration or
declared access, and full reloads start with fresh history. Changing a function's
body alone preserves history. A rejected candidate does not consume the previous
generation's history. `Local[T]` and message-reader cursors are not transferred.

Conditions attached to system sets refresh their callable, parameter plan and
caches together from the new configuration, including factory and nested
functions. Partial reload preserves their change-detection history and resets
`Local[T]`. The expression's operators and leaf identities, declared access and
thread requirements must remain unchanged. Incompatible changes reject the
candidate before cleanup in either mode and keep the previous scene running;
use `run_scene` to rebuild that schedule configuration.

Partial mode does not reconstruct retained Startup-owned state. These
consequences matter during iteration:

- Changing annotated `@resource` fields escalates to Full and rebuilds the
  resource. Changing only a default value keeps the current instance on Partial;
  use Full to apply the new initial value.
- Methods, properties and field defaults of `@component` and `@resource`
  classes follow the edited source on Partial. The class object and existing
  instances are kept, so a changed default applies to instances created after
  the reload.
  This includes user `__new__` and `__setattr__` edits and deleted constants
  or cached properties. Construction guards and resource assignment validation
  remain active. Rejected candidates leave the previous class body unchanged.
- When the field layout is compatible, the preserved resource accepts nested
  class and enum values declared by the successfully loaded generation. Existing
  field objects are not migrated: an old enum value remains an old enum value
  until scene code or `set_resource` assigns a current-generation replacement.
  Explicit enum writes through `set_resource` use the latest committed class for
  both a variant name and a tagged `{"variant": "NAME"}` value. Rejected reloads
  do not approve their candidate types for later assignment.
- Preserved custom-component fields follow the same no-migration rule. An MCP
  component update that names an enum member, including a tagged
  `{"variant": "CALM"}` value, constructs the replacement from the successfully
  committed generation even while the field still stores an old enum value.
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
get_last_error                     - Get Python error traceback if reload failed, plus wgpu_validation GPU errors, shader_errors, and asset_load_failures
```

`reload` and `reload_and_capture` answer after the reload they requested has run:
`mode` repeats the request, while `actual_mode`, `escalated` and `generation` describe
the reload that ran, as `get_reload_status` does. A Partial request that escalated
reports `actual_mode: "full"`. When the reload does not run within 60 frames,
the response has `status: "reload_requested"` and a `note`; read `get_reload_status`
for its outcome.
In an app running without hot reload, such as a script started with plain
`python scene.py`, both tools fail with an error and change nothing, including
the requested `pause` and `time_scale`.

`get_last_error.shader_errors` is `null` or lists the compile errors of shaders
whose pipelines currently fail, such as a WESL syntax error. An error in an
`@material` shader starts with its owner, for example
`@material GlowMaterial fragment shader shaders/glow.wesl (entities 12v0): `,
naming the class, the stages that use the file, and the entities drawing it.
It clears when the fixed shader recompiles, and the log then reports
`@material GlowMaterial shader shaders/glow.wesl compiles again`; a `.wesl`
edit applies without a Python reload.

`get_last_error.wgpu_validation` is `null` or reports GPU validation errors
recorded since the last successful reload: `count`, `first` and `last`
(`message`, pipeline `label`), and `last_seen_secs_ago`. A successful full or
partial reload clears it. An invalid pipeline reports again every frame, so a
still-broken shader reappears right after the reload, while a growing
`last_seen_secs_ago` means the error is no longer occurring.

Error responses from `reload` and `reload_and_capture` can include an advisory
`hint` when the surfaced exception has a recognized message. The field is
omitted when the message does not provide enough evidence. In particular, an
unpack error may suggest `Query[tuple[A, B]]` conditionally because ordinary
Python code can produce the same error outside a query. Treat hints as a next
check, and use the paired `error` and `traceback` as the source of truth.

`get_logs` retains up to 500 captured stdout and stderr entries for the current
scene subprocess. Watcher reloads, `reload`, and `reload_and_capture` all keep
that history, with consecutive duplicate lines summarized by a repeat count.
Calling `run_scene` stops or replaces the subprocess and starts a fresh log
history.

## Workflow: Edit and Reload

Scenes launched through `run_scene` are watched. Saving a Python file queues a
Partial reload by default, so do not call `reload` after every edit. Structural
changes may automatically escalate that request to Full.

1. Edit and save the Python source file
2. Check `get_reload_status` for a new generation or error, then use a capture tool to inspect a successful update
3. If errors occur, use `get_last_error` for the Python traceback

Do not use `reload_and_capture` just to inspect an edit the watcher has already
loaded; it also requests another reload. Use `reload {"mode": "full"}` after saving when you specifically need a clean
reset. A file-watcher request may already have run by then; an explicit Partial
reload cannot restore state that an earlier Full reload cleared. The CLI's F6
shortcut changes the mode used by later file saves.

The watcher skips a save whose file contents a reload has already executed, so
saving and then calling `reload` loads the edit once, and re-saving unchanged
contents does nothing. To retry the same contents, call `reload`. After a
failed reload, saving the contents that are still running reloads them and
clears the failure.

For CLI sessions, `pybevy dev scene.py --full` and `pybevy watch scene.py --full`
start the watcher in Full mode; without `--full`, both start in Partial mode.
This selects the initial mode only, so F6 still applies to later saves.
Pass scene arguments after `--`, for example
`pybevy dev scene.py --full -- --level 3`; the scene receives them in `sys.argv`
on initial load and subsequent reloads. Keep PyBevy options before `--`.

## Keyboard Shortcuts (CLI hot-reload mode)

If a reload fails while importing a new project dependency, repairing that
dependency triggers a retry without touching the importing scene. Failed
module/class changes remain rolled back; only dependency paths are retained
for retry planning until a reload succeeds.

| Key | Action |
|-----|--------|
| **F5** | Trigger Full reload |
| **F6** | Toggle between Full and Partial mode |
| **F7** | Toggle memory overlay (RSS, GC objects, per-reload deltas) |

`HotReloadControl.request_full_reload()` and `request_partial_reload()` also
queue a reload immediately; they do not wait for a file edit. Call them on an
edge such as `just_pressed`, not unconditionally every frame. A reload
reconstructs `Local` values, module globals, and function attributes, so a
"once" guard kept in those places can request another reload forever. If a
system needs both keyboard state and `HotReloadControl`, take `World` as its
only parameter and read both resources from it. `World` cannot be combined
with a separate `Res` parameter in the same system.

<!-- pybevy-snippet: typecheck -->
```python
from pybevy.app import App, DefaultPlugins, HotReloadControl, Update
from pybevy.decorators import entrypoint
from pybevy.ecs import World, system
from pybevy.input import ButtonInput, KeyCode


@system
def handle_f5(world: World) -> None:
    if world.resource(ButtonInput[KeyCode]).just_pressed(KeyCode.F5):
        world.resource(HotReloadControl).request_full_reload()


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Update, handle_f5)
```

## When to Use Each Mode

| Scenario | Mode | Why |
|----------|------|-----|
| Changed Startup system (scene setup), or data and helpers it reads | Full | Need to re-run Startup to apply |
| Changed Update system logic | Partial | Faster, keeps scene state |
| Added new `@component`/`@resource` types | Full | Need fresh registration |
| Changed `@component` fields or storage mode | Full | Partial reload auto-escalates before applying the new layout |
| Deleted a `@component` class an entity still carries | Full | Partial reload auto-escalates; `escalation_reason` names the class |
| Changed `@resource` fields | Full | Partial reload keeps the live instance, so it auto-escalates to rebuild it |
| Removed or renamed the active `@state` member | Full | Partial reload auto-escalates and reinitializes that state from the entrypoint |
| Tweaking animation parameters | Partial | Preserves entity state |
| First Partial reload after normal scene launch | Partial | Initial definitions provide the comparison baseline |
| Added/changed observers | Full | Auto-escalated from Partial |
| Renamed/removed a system function | `reload` | Removed registrations retire, then compact after the rollback window |
| Edited a `.wesl` shader or other asset | none | Bevy re-loads the asset itself; the Python watcher ignores non-`.py` files |
| Scene looks broken | Full | Clean slate |

Asset files are watched while hot reload is active, so editing a shader or texture
updates the running scene with no `reload` call. Asset paths resolve against the
directory the scene process was launched from, not the scene file's own directory,
so editing a same-named file next to the scene changes nothing.

The Python watcher uses the same project root as module flushing. For a scene
inside the launch directory that root is the launch directory; for an absolute
scene outside it, the root is the scene's directory. An edit reloads the scene
only when it changes the scene itself or a statically imported dependency.
This includes a new module after the scene is edited to import it and that
scene reload succeeds; later edits to the new module are watched normally.
Unrelated Python files under the root are ignored. Scenes that use dynamic
imports or load and execute Python files by path reload conservatively for
Python edits under the root. This includes `runpy`, `spec_from_file_location`,
and the `exec`/`compile` builtins; the watcher cannot infer a specific
dependency from a runtime path. Asset files such as `.wesl` shaders remain owned by
Bevy's asset watcher.

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

Rejected component and resource schema edits leave the last successful layout
and cached classes intact. Reverting to that schema keeps a subsequent Partial
reload partial and preserves live state. A successfully loaded schema change
still escalates to Full.

If a reload introduces a Python error:
1. Import and entrypoint failures keep the previous world running. A
   non-escalated Partial registration failure does too. Full reload clears the
   scene before registration and Startup, so failures after that point roll
   back the active code generation but cannot restore the previous world. In
   that case old generation-gated systems can run against the emptied World,
   while `running_previous_generation` is false because the prior scene is not
   intact. A candidate whose `before`/`after`, `chain`, `in_set`, or
   `configure_sets` ordering would stop a schedule from building, such as a
   cycle, is rejected before cleanup in both modes. The previous world keeps
   running and the error names the cycle. The previous generation's systems
   stay scheduled beside the candidate's, so reversing the order of two
   existing systems in one reload is reported as a cycle; use `run_scene` for
   that edit.
2. Call `get_last_error` to see the traceback
3. Fix the Python source
4. Call `reload {"mode": "full"}` to retry

After a failed Full reload, the next reload is a Full recovery whatever was
requested or saved, and it re-imports every project module, including ones the
failed attempt had rolled back. Fixing a different file than the one that broke
the reload is therefore enough.

Failed reloads restore the previous scene namespace and helper module
registrations, so `run_code` can inspect the previous generation's definitions.
The previous world survives only when failure occurs before Full cleanup or on
a non-escalated Partial reload. This does not undo arbitrary import-time side
effects, such as file writes.

`running_previous_generation` means the previous generation is running with
its scene World intact. It is not merely a code-generation check. After a
failed reload, the `reload` and `reload_and_capture` responses,
`get_reload_status`, `get_last_error` and `get_scene_summary` all report it,
so an empty summary with `running_previous_generation: false` is a scene the
failed Full reload cleared. Each of them then also adds `scene_cleared`, saying
so in words.

After successful recovery, the debug overlay clears the failure indicator and
hides the previous error. Full reload resets the scene clocks but does not stall
overlay or performance-statistics refreshes; errors in the new scene replace
the displayed error even when their timestamps restart at zero.

Each distinct runtime system error is printed to stderr at most once per
registered system generation. Successful calls between repeats do not reset
this suppression. `get_last_error` exposes the latest failure even after a
failed reload, while `get_reload_status` retains that reload attempt's result.
SSE events (`/mcp/v1/sse`) broadcast errors in real-time.

Code that never returns, such as `while True:` in a system, an observer or a
module imported by a reload, holds the scene's main thread. No reload, fix or
control call can interrupt it, and tools time out. The timeout then says
`The scene's main thread has been inside one frame for Ns`, which the bridge
reads from `GET /api/v1/engine/activity`; that endpoint answers from the control
server's own thread. Fix the code and call `run_scene` to restart. `run_code`
has its own deadline and does not need this.

## Type Re-aliasing (`@component` / `@resource`)

Custom Python types defined with `@component` or `@resource` survive hot reloads via **name-based re-aliasing**:

- After reload, Python classes get new `PyTypeObject` pointers
- The reload system matches new types to existing `ComponentId`s by qualified name (e.g., `__main__.Player`)
- If the structure matches (same storage mode, same fields), the existing `ComponentId` is reused - no data loss
- If the structure changes (e.g., added a field, switched storage mode), a fresh `ComponentId` is allocated
- Outside a reload, a second same-named `@component` class with different fields raises `TypeError` at spawn or insert, so the first class's entities stay intact

This means `reload` with Full mode handles most `@component`/`@resource` changes. Use `run_scene` only when you need a guaranteed clean-slate restart.

Custom message classes are re-aliased by `module.qualname` to the same logical
channel. Obsolete exact class aliases expire after the supported window of one
previous successful reload generation. Removed or renamed channels retain their
neutral identity and ID sequence without retaining their old Python classes.

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

A re-registered observer starts with fresh change-detection history: its first
run after the reload treats every matching entity as `Changed`/`Added`. Kept
observers keep their history.

Full reload clears the whole Python observer registry before Startup re-runs, so
a `world.add_observer` registration is replaced rather than duplicated. Entity
observers are removed with their targets and must be registered again when
Startup creates the replacement entities.

Partial reload replaces only observers declared by `app.add_observer` in the
entrypoint. Their callable and custom-event class therefore follow the committed
scene generation without duplicate registrations. Observers registered later by
`world.add_observer` or `entity.observe` remain attached to their target, just as
their runtime entities remain in the World, and keep receiving their custom
event: an `@event` class keeps its identity across partial reloads and takes the
reloaded class body, as `@component` and `@resource` classes do. An `Event`
subclass without `@event` is a new class after each reload, so runtime observers
of it stop firing.

Entity-targeted dispatch indexes observers by event and target, so unrelated
entity observers are not scanned for each trigger. Global observers still run
alongside matching entity observers in registration order.

## Plugin Delta Detection

When plugins are added or removed across reloads, the reload system detects the delta:

- **New plugins**: Reported in `get_reload_status` as `plugins_added`
- **Removed plugins**: Reported as `plugins_removed` (restart may be required)
- Core Bevy plugins (DefaultPlugins, etc.) cannot be hot-removed - a restart is needed

The delta compares with the last successfully committed generation, starting
with the initial App declarations, and is reported once. An unchanged reload
reports neither field, and a failed candidate does not publish or advance the
comparison baseline.

A reload whose `@entrypoint` code changed reports `entrypoint_changed` once, with
an explanation. Python plugins, systems and observers declared there follow the
reload, but native plugin settings such as `ScheduleRunnerPlugin.run_loop(8)`,
`WindowPlugin` and other App configuration apply only at startup, so call
`run_scene` for them. Comment and blank-line edits that only move the entrypoint
do not count.

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

Inside a reloading `build()`, `app.is_plugin_added(X)` reports whether the
reloading entrypoint has added `X` so far, so the usual
`if not app.is_plugin_added(X): app.add_plugins(X())` guard behaves the same
on the first load and on reload.

## Selective Module Flushing

Hot reload uses an AST-based import graph to minimize the modules flushed from `sys.modules`:

- On startup, all `.py` files under the watch root are parsed and import dependencies are tracked
- Imports through `sys.path` entries inside the watch root are tracked too;
  edits to a helper in a project-local `lib/` or `src/` directory can reload the scene
- The start-up scan skips the same directories as the file watcher, including
  dot-directories, which stay untracked even through a `sys.path` entry. Installed
  code is never tracked: a Python environment below the watch root (marked by
  `pyvenv.cfg` or `conda-meta/`) or the interpreter's install roots, even when
  imported through `sys.path`
- When files change, only the changed files and their transitive dependents are flushed
- Package initializers are dependencies of modules inside their package; editing
  `pkg/__init__.py` flushes cached `pkg.*` modules before re-importing them
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
- Warning indicator if RSS growth exceeds threshold (200 MB default)

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

`memory_growth_mb` is RSS since a baseline taken after the first 120 frames, not
reload leak: asset loading after that point still counts, so it can read tens or
hundreds of MB at `reload_count: 0`. It is absent until the baseline exists. For
per-reload growth use `delta_mb` in `reload_memory_snapshots`.

Retired views release their bin-unpacking bind groups and cached pipeline keys during render cleanup;
allocator and driver caching can still keep reported memory above its startup level.

## Reload Schedule Boundary

Reload maintenance runs after `Update`, before `PostUpdate` and `Last`.
A pending request therefore leaves `PreUpdate` and `Update` in the previous
generation on the request frame; after a successful commit, `PostUpdate` and
`Last` use the new generation in that same frame. Full also runs the replacement
startup schedules during maintenance. Requests made in `PostUpdate` or `Last`
wait for the next maintenance pass. This boundary prepares replacement scene
assets before native `PostUpdate` and render extraction.

Failed loader calls keep the previous generation running. Diagnostics preserve
exception chains and groups, but saved loader exceptions have their traceback
frames detached so they cannot retain the temporary reload App.

## System Rename/Removal Detection

After a successful reload, removed or renamed callable names are logged and
reported by `get_reload_status` as `systems_removed`. Failed candidates do not
change this comparison baseline. The field is a distinct-name delta for that
successful reload, so registering the same callable more than once does
not repeat its name.

Bevy schedule nodes are retained for one rollback generation, then physically
compacted during reload maintenance when the next generation commits. This
applies even when the new generation registers no systems. Generation guards
keep predecessor nodes from executing. During that bounded window,
`get_system_list()` reports:

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
| checking the candidate's schedule ordering | previous run's entities still there |
| registering a non-escalated Partial reload | previous run's entities still there |
| registering a Full reload | scene emptied - entities were already cleared |
| running a Full-reload Startup system | scene emptied - entities are cleared *before* Startup re-runs |

Always check `get_last_error` first. A full `run_scene` gives a guaranteed clean state.

### GLB textures not loaded after reload_and_capture

`reload_and_capture` waits for a post-reload render with no queued shader
pipelines before counting `delay_frames` (default 30). This also applies when
`delay_frames=0`, and works with paused time. It does not wait for asynchronous
asset loads: GLB textures and meshes may still arrive after that render. For
scenes with GLB models, use `delay_frames=90` or higher. If textures still appear
missing (e.g., `Image:4` instead of `Image:15`), use `run_scene` followed by
`capture_screenshot` with a higher delay.
