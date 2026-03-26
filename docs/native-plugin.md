# Using PyBevy as a Native Bevy Plugin

`PyBevyPlugin` integrates Python systems into an existing Rust Bevy application — for adding modding support, scripting, or leveraging the Python ecosystem from a Rust codebase.

> **Security Warning:** `PyBevyPlugin` embeds a **full CPython interpreter** with unrestricted access to the host system — file I/O, network, subprocesses, and everything else Python can do. **Never execute untrusted Python code.** If you are considering user-submitted scripts (e.g., a modding/plugin system), be aware that Python has no built-in sandboxing. Any code you load runs with the same privileges as your application. We strongly advise against loading arbitrary user-provided Python scripts without a thorough security review and external sandboxing measures.

## Quick Start

### 1. Add PyBevy to your Rust project

```toml
# Cargo.toml
[dependencies]
bevy = "0.18"
pybevy = { version = "0.2", features = ["native-plugin"] }
```

### 2. Create your Python systems module

```python
# systems/game.py
import math
from pybevy.prelude import *
from pybevy.decorators import component
from pybevy.ecs import Commands, ResMut, With
from pybevy.assets import Assets
from pybevy.mesh import Mesh3d, MeshMaterial3d
from pybevy.render import StandardMaterial
from pybevy.math import Circle, Cuboid, Quat, Vec3
from pybevy.time import Time


@component
class Cube(Component):
    """Marker for the rotating cube."""
    pass


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Circular base
    commands.spawn(
        Mesh3d(meshes.add(Circle(radius=4.0).mesh())),
        MeshMaterial3d(materials.add(StandardMaterial.from_color(Color.WHITE))),
        Transform.from_rotation(Quat.from_rotation_x(-math.pi / 2)),
    )

    # Cube
    commands.spawn(
        Cube(),
        Mesh3d(meshes.add(Cuboid(1.0, 1.0, 1.0).mesh())),
        MeshMaterial3d(materials.add(StandardMaterial.from_color(Color.srgb_u8(124, 144, 255)))),
        Transform.from_xyz(0.0, 0.5, 0.0),
    )

    # Light and camera
    commands.spawn(PointLight(shadows_enabled=True), Transform.from_xyz(4.0, 8.0, 4.0))
    commands.spawn(Camera3d(), Transform.from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3.ZERO, Vec3.Y))


def rotate_cube(query: Query[Mut[Transform], With[Cube]], time: Res[Time]) -> None:
    for transform in query:
        transform.rotation *= Quat.from_rotation_y(time.delta_secs())
```

### 3. Use PyBevyPlugin in your Rust app

```rust
use pybevy::PyBevyPlugin;
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(
            PyBevyPlugin::new("game")
                .with_python_path("systems")
                .with_startup_system("setup")
                .with_update_system("rotate_cube")
                .with_hot_reload(),  // Edit Python, press F5 to reload!
        )
        .add_systems(Update, print_entity_count)
        .run();
}

fn print_entity_count(query: Query<Entity>, mut frame_count: Local<u32>) {
    *frame_count += 1;
    if *frame_count % 60 == 0 {
        println!("[Rust] Entity count: {}", query.iter().count());
    }
}
```

## API Reference

### `PyBevyPlugin::new(module_name)`

Create a new plugin that loads systems from the specified Python module.

```rust
PyBevyPlugin::new("my_game.systems")
```

If no systems are specified, it will auto-discover functions named `startup`, `update`, and `last`.

### `with_python_path(path)`

Add a directory to Python's `sys.path` for module resolution. Use this when your Python module is not in the current working directory.

```rust
PyBevyPlugin::new("game")
    .with_python_path("scripts/python")
    .with_startup_system("setup")
```

Without this, Python only looks in the current working directory and standard paths.

### `with_system(function_name, stage)`

Add a specific system function to a particular Bevy schedule.

```rust
use pybevy::PyStage;

PyBevyPlugin::new("systems")
    .with_python_path("scripts")
    .with_system("init_world", PyStage::Startup)
    .with_system("game_tick", PyStage::Update)
    .with_system("cleanup", PyStage::Last)
```

### `with_hot_reload()`

Enable hot reload. Python scripts can be modified while the app is running and reloaded without restarting.

```rust
PyBevyPlugin::new("game")
    .with_python_path("scripts")
    .with_startup_system("setup")
    .with_update_system("rotate_cube")
    .with_hot_reload()
```

When enabled:
- **F5** — Full reload (re-imports module, clears entities, re-runs Startup systems)
- **F6** — Toggle default reload mode between Full and Partial
- **File watcher** — Automatically reloads on `.py` file changes (requires `native-hot-reload` Cargo feature with `notify` crate)

See the [Hot Reload](#hot-reload) section below for details.

### Convenience methods

- `with_startup_system(name)` - Add to Startup schedule
- `with_update_system(name)` - Add to Update schedule
- `with_last_system(name)` - Add to Last schedule
- `with_auto_discovery()` - Automatically find `startup`, `update`, `last` functions

## Python System Signatures

Python systems use the same type annotation syntax as Python-first PyBevy. Use `from pybevy.prelude import *` for common types, then import specific types from submodules:

```python
from pybevy.prelude import *
from pybevy.ecs import Commands, Res, ResMut, With
from pybevy.time import Time

# System with commands and query
def my_system(
    commands: Commands,
    query: Query[Mut[Transform], With[Marker]],
    time: Res[Time],
) -> None:
    pass

# System with asset access
def load_assets(
    asset_server: AssetServer,
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    pass
```

**Important**: Resources must be wrapped in `Res[T]` (read-only) or `ResMut[T]` (mutable). Bare resource types like `time: Time` will not work.

## Mixing Rust and Python Systems

You can freely mix Rust and Python systems in the same application:

```rust
use pybevy::PyBevyPlugin;
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Rust systems
        .add_systems(Startup, setup_camera)
        .add_systems(Update, handle_input)
        // Python systems
        .add_plugins(
            PyBevyPlugin::new("gameplay")
                .with_python_path("scripts")
                .with_startup_system("spawn_enemies")
                .with_update_system("ai_behavior"),
        )
        // More Rust systems
        .add_systems(Last, cleanup_dead_entities)
        .run();
}
```

Both Rust and Python systems can read and write the same built-in components (Transform, PointLight, etc.). Custom components defined in Python (via `@component`) are only accessible from Python systems.

## Custom Rust Components (`#[derive(PyComponent)]`)

You can define components in Rust and make them queryable/mutable from Python:

### 1. Define the component

```rust
use pybevy::PyComponent;
use bevy::prelude::*;

#[derive(Component, Default, Clone, Debug, PyComponent)]
struct Health {
    value: f32,
    max: f32,
}
```

The derive macro generates:
- `PyHealth` — Python wrapper with getters/setters for all fields
- `HealthBridge` — Bridge struct for registering with PyBevy
- `register_health()` — Registration function

### 2. Register with PyBevyPlugin

```rust
App::new()
    .add_plugins(DefaultPlugins)
    .add_plugins(
        PyBevyPlugin::new("game_systems")
            .with_python_path("scripts")
            .register_component(HealthBridge)
            .with_startup_system("setup")
            .with_update_system("apply_damage"),
    )
    .add_systems(Startup, spawn_with_health)
    .add_systems(Update, print_health)
    .run();
```

### 3. Use from Python

Registered components are injected into `pybevy._pybevy`, so Python can import and use them like any built-in component:

```python
from pybevy.prelude import *
from pybevy._pybevy import Health

def apply_damage(query: Query[Mut[Health]], time: Res[Time]) -> None:
    for health in query:
        health.value -= 5.0 * time.delta_secs()
        if health.value < 0.0:
            health.value = health.max  # Reset
```

### 4. Rust reads the updated values

```rust
fn print_health(query: Query<&Health>, mut frame_count: Local<u32>) {
    *frame_count += 1;
    if *frame_count % 60 == 0 {
        for health in &query {
            println!("[Rust] Health: {:.1}/{:.1}", health.value, health.max);
        }
    }
}
```

### Supported field types

The derive macro auto-generates getters/setters for:
- Primitives: `f32`, `f64`, `i32`, `u32`, `bool`, etc.
- Non-primitives that implement `Into`/`From` conversions

### Attributes

- `#[py_name("CustomName")]` on the struct — override the Python class name (default: same as Rust name)

## How It Works

`PyBevyPlugin` embeds a Python interpreter into your Rust binary and registers the `_pybevy` extension module with it. This ensures that `from pybevy.prelude import *` in your Python code uses the same type objects as the running binary, so type annotations in system parameters resolve correctly.

The initialization flow:
1. `append_to_inittab!(_pybevy)` registers the module before Python starts
2. `Python::initialize()` starts the embedded interpreter
3. `sys.modules["pybevy._pybevy"]` is pre-populated so relative imports work
4. Your Python module is imported and system functions are extracted
5. Each function is wrapped as a `DynamicSystem` and added to Bevy schedules

## Hot Reload

Hot reload lets you modify Python scripts while the app is running and see changes without restarting. Enable it with `.with_hot_reload()`:

```rust
PyBevyPlugin::new("game")
    .with_python_path("scripts")
    .with_startup_system("setup")
    .with_update_system("rotate_cube")
    .with_hot_reload()
```

### Reload triggers

| Key / Trigger | Action |
|---------------|--------|
| **F5** | Full reload — clears entities, re-imports module, re-runs Startup |
| **F6** | Toggle default mode between Full and Partial |
| **File change** | Auto-reload on `.py` save (requires `native-hot-reload` feature) |

### Reload modes

- **Full reload** — Clears user-spawned entities and custom resources, re-imports the Python module, re-runs Startup systems. Use when you've changed scene setup code.
- **Partial reload** — Keeps all entities and resources, only re-imports Update/Last systems. Use when iterating on game logic.

### Auto-reload with file watcher

To enable automatic reload on file save, add the `native-hot-reload` feature to your `Cargo.toml`:

```toml
[dependencies]
pybevy = { version = "0.2", features = ["native-hot-reload"] }
```

This uses the `notify` crate to watch directories specified via `with_python_path()`. When a `.py` file changes, a reload is triggered using the current default mode (toggled with F6).

### Example workflow

1. Run your app: `cargo run --release`
2. Open `scripts/game.py` in your editor
3. Change `ROTATION_SPEED = 1.0` to `ROTATION_SPEED = 5.0`
4. Press F5 (or save if file watcher is enabled)
5. The cube now spins 5x faster — no restart needed

## Performance Considerations

- **Python GIL**: Python systems acquire the Global Interpreter Lock when executing
- **Interop overhead**: There's overhead converting between Rust and Python types
- **Recommendation**: Use Python for game logic/AI/scripting, Rust for performance-critical systems (physics, rendering, etc.)

## Error Handling

- If a Python module fails to import, a warning is printed but the app continues
- If auto-discovery is used, missing functions are silently skipped
- Runtime errors in Python systems are printed to stderr

## Complete Example

See `examples/native/native_plugin_example.rs` and `examples/native/example_systems.py` for a complete working example:

```bash
cargo run --example native_plugin_example
```

## Troubleshooting

### "Failed to import module"

Use `with_python_path()` to tell Python where to find your module:

```rust
PyBevyPlugin::new("my_systems")
    .with_python_path("path/to/python/modules")
```

Or set `PYTHONPATH`:

```bash
export PYTHONPATH=/path/to/python/modules:$PYTHONPATH
cargo run --example native_plugin_example
```

### "Unsupported system parameter"

Make sure your Python imports use the submodule pattern, not bare `from pybevy import ...`:

```python
# Correct
from pybevy.prelude import *
from pybevy.ecs import Commands, Res, ResMut

# Wrong - types may not be exported from top-level
from pybevy import Commands, Query, Transform
```

### "Must use Res[T] for read-only or ResMut[T] for mutable access"

Resources must be wrapped:

```python
# Correct
def my_system(time: Res[Time]) -> None: ...

# Wrong
def my_system(time: Time) -> None: ...
```

### "Module has no function"

Check that your Python function names match what you specified in `with_system()`:

```rust
// Rust side
.with_update_system("update_game")
```

```python
# Python side - names must match!
def update_game(query: Query[Mut[Transform]]) -> None:
    pass
```

## Limitations

- `#[derive(PyComponent)]` only supports structs with named fields (not enums, tuples, or unit structs)
- Python systems cannot use advanced Bevy scheduling features (ordering, run conditions)
- System ordering between Rust and Python systems uses standard Bevy ordering
