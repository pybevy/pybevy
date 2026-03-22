# Hot Reload Guide

Full vs partial reload, what persists across reloads, error recovery, and diagnostics.

## Reload Modes

### Full Reload
- Clears all entities and custom Python resources
- Preserves built-in resources (Time, AssetServer, WireframeConfig, GlobalVolume)
- Re-runs ALL systems including Startup (re-creates the scene)
- Entity IDs will change — use `Name` component for stable references
- Custom `@component` and `@resource` types are re-aliased by name (no new ComponentId if structure unchanged)
- Observers are re-registered automatically
- Old DynamicSystems are cleaned up to prevent memory leaks

### Partial Reload
- Preserves all entities and resources (including custom ones)
- Only reloads Update/Last system functions
- Startup systems are NOT re-run
- Use for iterating on game logic without resetting scene state
- Auto-escalates to Full if observers are pending

## MCP Reload Commands

```
reload {"mode": "full"}    — Full reload (default)
reload {"mode": "partial"} — Partial reload

# Atomic time control with reload (avoids timing drift between separate calls):
reload {"mode": "full", "pause": true}                — Reload and freeze immediately
reload {"mode": "full", "time_scale": 0.1}            — Reload at slow-motion
reload {"mode": "full", "pause": true, "time_scale": 0.1} — Both: frozen at 0.1x, resume when ready

get_reload_status                  — Check if reload is pending/complete
get_last_error                     — Get Python error traceback if reload failed
```

## Workflow: Edit and Reload

1. Edit the Python source file (e.g., `examples/mcp/mcp_scene.py`)
2. Call `reload {"mode": "full"}` to apply changes
3. Wait briefly for reload to complete
4. Call `capture_screenshot` to verify the result
5. If errors: `get_last_error` shows the Python traceback

## Keyboard Shortcuts (CLI hot-reload mode)

| Key | Action |
|-----|--------|
| **F5** | Trigger reload (uses current mode) |
| **F6** | Toggle between Full and Partial mode |
| **F7** | Toggle memory overlay (RSS, GC objects, per-reload deltas) |

## When to Use Each Mode

| Scenario | Mode | Why |
|----------|------|-----|
| Changed Startup system (scene setup) | Full | Need to re-run Startup to apply |
| Changed Update system logic | Partial | Faster, keeps scene state |
| Added new `@component`/`@resource` types | Full | Need fresh registration |
| Changed `@component` field structure | Full | Re-aliasing detects structural change |
| Tweaking animation parameters | Partial | Preserves entity state |
| Added/changed observers | Full | Auto-escalated from Partial |
| Renamed/removed a system function | `run_scene` | Stale schedule entries persist across `reload` |
| Scene looks broken | Full | Clean slate |

## Entity ID Stability

Entity IDs are NOT stable across full reloads. Always use `Name` components:

```python
# In Python source
commands.spawn(Transform(), PointLight(intensity=1000), Name("sun"))
```

```
# In MCP — address by name
set_component {"name": "sun", "component": "PointLight", "fields": {"intensity": 2000}}
```

## Error Recovery

If a reload introduces a Python error:
1. The scene may freeze or show stale state
2. Call `get_last_error` to see the traceback
3. Fix the Python source
4. Call `reload {"mode": "full"}` to retry

SSE events (`/mcp/v1/sse`) also broadcast errors in real-time.

## Type Re-aliasing (`@component` / `@resource`)

Custom Python types defined with `@component` or `@resource` survive hot reloads via **name-based re-aliasing**:

- After reload, Python classes get new `PyTypeObject` pointers
- The reload system matches new types to existing `ComponentId`s by qualified name (e.g., `__main__.Player`)
- If the structure matches (same storage mode, same fields), the existing `ComponentId` is reused — no data loss
- If the structure changes (e.g., added a field, switched storage mode), a fresh `ComponentId` is allocated

This means `reload` with Full mode handles most `@component`/`@resource` changes. Use `run_scene` only when you need a guaranteed clean-slate restart.

## Plugin Delta Detection

When plugins are added or removed across reloads, the reload system detects the delta:

- **New plugins**: Reported in `get_reload_status` as `plugins_added`
- **Removed plugins**: Reported as `plugins_removed` (restart may be required)
- Core Bevy plugins (DefaultPlugins, etc.) cannot be hot-removed — a restart is needed

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
- `DefaultPlugins` and other `PluginGroup` types — these persist from the initial load
- Bridge-backed plugins (built-in Rust plugins) — they call `with_bevy_app()` which temp apps lack
- For new plugin *types* added mid-development (not just new instances), `run_scene` restart may still be needed

## Selective Module Flushing

Hot reload uses an AST-based import graph to minimize the modules flushed from `sys.modules`:

- On startup, all `.py` files under the watch root are parsed and import dependencies are tracked
- When files change, only the changed files and their transitive dependents are flushed
- This makes reloads faster for large projects (unchanged modules stay cached)
- F5 with Full mode always does a complete entity reset regardless of module flushing scope

## Memory Profiling

Press **F7** to toggle the memory overlay, which shows:

- Current RSS (resident set size) and growth since baseline
- Python GC object count
- Schedule system count
- Per-reload memory deltas (rolling window of last 20 reloads)
- Warning indicator if RSS growth exceeds threshold (100 MB default)

Memory data is also available via MCP:
```
get_performance  — includes memory_growth_mb, memory_peak_mb, memory_warning, reload_memory_snapshots
```

## System Rename/Removal Detection

When systems are renamed or removed across reloads, the reload system detects the delta:

- **Removed/renamed systems**: Logged as a warning and reported in `get_reload_status` as `systems_removed`
- Stale schedule entries from old systems remain in Bevy's schedule graph (Bevy does not support removing individual systems)
- Old systems are disabled via generation guards and will not execute, but may appear in schedule conflict error messages
- **Use `run_scene`** (not `reload`) to fully clear stale system registrations

## Time Continuity

`time.elapsed_secs()` is **NOT reset** by hot-reload — it continues from the app start time. Scenes that compute positions from elapsed time (e.g., `position = time.elapsed_secs() * speed`) will accumulate large offsets across reloads.

**Workarounds:**
- Use modular/wrapping position logic so entities stay near the camera
- Use delta-time accumulation in a Resource instead of absolute elapsed time
- Use `run_scene` for a full restart (resets time to 0)

## Troubleshooting

### "resource not found in world"

- **Startup crashed before `insert_resource()`** — If setup errors mid-execution (e.g., wrong attribute name), resources inserted after the error line are never created. Check `get_last_error` or `get_logs(errors_only=true)` for tracebacks. Fix the error and use `run_scene` (not `reload`) to get a clean start.
- **Missing `app.insert_resource()`** — If your Startup system takes `ResMut[MyResource]`, the resource must already exist. Add `app.insert_resource(MyResource())` in your `@entrypoint` before `.add_systems(Startup, setup)`.
- **Used `reload` after changing `@component` field structure** — Adding/removing fields or changing storage mode allocates a new `ComponentId`. Full reload handles this (entities are recreated), but if behavior is unexpected, use `run_scene` for a clean restart.

### Stale entities after failed reload

After a failed reload, entities from the previous run may still exist. Don't use entity counts to judge whether Startup succeeded — always check `get_last_error` first. A full `run_scene` gives a guaranteed clean state.

### GLB textures not loaded after reload_and_capture

`delay_frames` is frame-based, not asset-aware. GLB models trigger async texture/mesh loads that may not complete within the default frame delay. For scenes with GLB models, use `delay_frames=90` or higher. If textures still appear missing (e.g., `Image:4` instead of `Image:15`), use `run_scene` followed by `capture_screenshot` with a higher delay.
