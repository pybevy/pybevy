# PyBevy: A Python Real-Time Engine Built on Bevy

[![Discord](https://img.shields.io/discord/1282043096002986034.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/hA4zUneA8f)
[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/pybevy/pybevy#license)
[![pypi](https://img.shields.io/pypi/v/pybevy)](https://pypi.org/project/pybevy/)
[![pypi downloads](https://img.shields.io/pypi/dm/pybevy)](https://pypi.org/project/pybevy/)

```bash
$ pip install pybevy
```

> **Beta.** Python 3.12+. API evolving — breaking changes expected.
> Independently developed, community-maintained. Not affiliated with the Bevy project.

**[pybevy.com](https://pybevy.com)** — Project website & documentation

Write Python, save the file, and see your 3D scene update. Use NumPy, JAX, and PyTorch in the same process as a real 3D renderer. And when you want it, the AI can join the loop — it writes code, sees the result, and iterates.

- **Fast hot reload** — edit code, see changes near instantly, no restart, no recompile
- **Built on Bevy's renderer and ECS** — PBR, volumetric fog, cascaded shadows, bloom, SSAO
- **Python ecosystem in-process** — NumPy, JAX, PyTorch, Numba — just import
- **Optional AI feedback loop** — the AI writes Python, reloads, inspects the running scene, and iterates
- **If you know Bevy's Rust API**, PyBevy should feel immediately familiar

![Hot reloading demo](https://raw.githubusercontent.com/pybevy/pybevy/main/assets/demo.webp)

## Getting Started

### Installation

Pre-compiled wheels are available for the following platforms:

| Platform | Architecture                |
| -------- | --------------------------- |
| Linux    | x86_64                      |
| macOS    | ARM (Apple Silicon), x86_64 |
| Windows  | x86_64                      |

> **Browser sandbox** is in development — follow [#12](https://github.com/pybevy/pybevy/issues/12) for progress.

```bash
pip install pybevy --upgrade
```

#### Linux System Dependencies

PyBevy's pre-built wheels link against system display and audio libraries.
On most desktop distributions these are already present.

If you see an `ImportError` mentioning `libwayland-client.so` or `libasound.so`:

```bash
# Debian/Ubuntu
sudo apt install libwayland-client0 libasound2t64

# Fedora/RHEL
sudo dnf install alsa-lib wayland
```

**Docker / headless environments** also need a software GPU driver:

```bash
apt install -y libwayland-client0 libasound2t64 mesa-vulkan-drivers
```

ALSA warnings about missing audio devices are harmless and can be ignored.

### Free-Threaded Python (3.13t+)

PyBevy supports Python's free-threaded mode (PEP 703). Non-conflicting Python systems run truly in parallel on separate cores via Bevy's multi-threaded scheduler — no GIL serialization. Validated on CPython 3.14t. Performance depends on workload and scene complexity; see [Benchmarks](docs/benchmarks.md) for methodology and numbers.

> **Note:** The Numba JIT path (View API Tier 3) is not yet available on free-threaded Python — `llvmlite` does not ship 3.14t wheels yet. Tracked in [#8](https://github.com/pybevy/pybevy/issues/8).

## Quick Example

Parent-child entity hierarchy with a rotating parent cube. The child cube inherits the parent's transform automatically.

```python
from pybevy.decorators import component, entrypoint
from pybevy.prelude import *


@component
class Rotator(Component):
    """Marks entities that should rotate."""


def rotator_system(
    time: Res[Time],
    query: Query[Mut[Transform], With[Rotator]]
) -> None:
    for transform in query:
        transform.rotate_x(3.0 * time.delta_secs())

def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    cube_handle = meshes.add(Cuboid(2.0, 2.0, 2.0))
    cube_material = materials.add(StandardMaterial(
        base_color=Color.srgb(0.8, 0.7, 0.6)
    ))

    # Parent cube — rotates
    parent = commands.spawn(
        Mesh3d(cube_handle),
        MeshMaterial3d(cube_material),
        Transform.from_xyz(0.0, 0.0, 1.0),
        Rotator(),
    )

    # Child cube — follows the parent
    parent.with_children(
        lambda child: [
            child.spawn(
                Mesh3d(cube_handle),
                MeshMaterial3d(cube_material),
                Transform.from_xyz(0.0, 0.0, 3.0),
            )
        ]
    )

    commands.spawn(PointLight(), Transform.from_xyz(4.0, 5.0, -4.0))
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(5.0, 10.0, 10.0).looking_at(Vec3.ZERO, Vec3.Y),
    )


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, rotator_system)
    )

if __name__ == "__main__":
    main().run()
```

Save this as `main.py` and run with hot reload:

```bash
pybevy watch --full main.py
```

Edit the code — the engine hot reloads near instantly, no restart, no recompile.

The `--full` flag reloads everything on each change, including setup systems. Without it, only Update systems are reloaded — faster for iterating on runtime behavior once your scene is set up.

### Native Plugin Approach (Rust + Python)

If you already have a Rust Bevy application, you can embed Python systems into it with `PyBevyPlugin`. Prototype in Python, ship critical paths in Rust.

> **Security:** `PyBevyPlugin` embeds a full CPython interpreter with unrestricted host access. Never execute untrusted Python code. We advise against using this for user-submitted plugin/modding systems without external sandboxing.

```rust
// main.rs - Your existing Rust Bevy application
use bevy::prelude::*;
use pybevy::PyBevyPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Add Python systems from a module
        .add_plugins(
            PyBevyPlugin::new("game_logic")
                .with_startup_system("setup")
                .with_update_system("ai_behavior")
                .with_hot_reload()  // Edit Python, press F5 to reload!
        )
        // Mix with native Rust systems
        .add_systems(Update, physics_step)
        .run();
}
```

```python
# game_logic.py
from pybevy.prelude import *

ROTATION_SPEED = 1.0  # Change this and press F5!

def setup(commands: Commands) -> None:
    commands.spawn(Transform.from_xyz(0.0, 0.0, 0.0))

def ai_behavior(query: Query[Mut[Transform]], time: Res[Time]) -> None:
    for transform in query:
        transform.rotation *= Quat.from_rotation_y(time.delta_secs() * ROTATION_SPEED)
```

See [docs/native-plugin.md](docs/native-plugin.md) for the full guide, including `#[derive(PyComponent)]` for exposing Rust components to Python and hot reload details.

### AI Feedback Loop

PyBevy includes a built-in [MCP server](https://modelcontextprotocol.io/) that lets AI agents write Python, reload the scene, capture screenshots, inspect entities, and iterate — automatically. The AI sees what it builds.

```bash
claude mcp add pybevy -- pybevy mcp   # Claude Code
codex mcp add pybevy -- pybevy mcp    # Codex
gemini mcp add pybevy -- pybevy mcp   # Gemini CLI
```

Also works with Cursor and other MCP-compatible editors. See [docs/mcp.md](docs/mcp.md) for full setup.

## Bevy Compatibility

PyBevy versions target specific Bevy versions:

| pybevy | Bevy |
| ------ | ---- |
| 0.2.x  | 0.18 |
| 0.1.x  | 0.18 |

PyBevy follows Bevy's API conventions as closely as possible and targets full coverage of Bevy's public API. Core modules like transforms, lighting, cameras, and input are fully covered; others are in progress. See [API Coverage](docs/coverage.md) for current per-module stats and [Limitations](docs/limitations.md) for known constraints.

## Limitations

- **No built-in physics** — use NumPy or JAX for physics computation, PyBevy for visualization.
- **Desktop only** — Linux, macOS, Windows. No mobile.
- **Code only** — no visual editor.
- **API is evolving** — see [Limitations](docs/limitations.md) for known constraints.

## Documentation

- **[pybevy.com](https://pybevy.com)** — Project website
- **[Examples](https://github.com/pybevy/pybevy/tree/main/examples)** — Runnable examples covering 2D, 3D, ECS, animation, and more
- **[API Coverage](docs/coverage.md)** — Per-module Bevy API coverage stats
- **[Limitations](docs/limitations.md)** — Known limitations

## Community & Contributing

- **Discord:** [Pybevy Discord](https://discord.gg/hA4zUneA8f) — questions, discussion, showcases
- **Contributing:** See [CONTRIBUTING.md](https://github.com/pybevy/pybevy/blob/main/CONTRIBUTING.md)

## License

All code in this repository is dual-licensed under either:

- [MIT License](https://github.com/pybevy/pybevy/blob/main/LICENSE-MIT)
- [Apache License 2.0](https://github.com/pybevy/pybevy/blob/main/LICENSE-APACHE)

at your option.

By contributing, you agree your work will be released under both licenses.

---

_When you want it, the world runs itself._
