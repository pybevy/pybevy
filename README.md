# PyBevy: A Python Real-Time Engine Built on Bevy

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/pybevy/pybevy#license)
[![pypi](https://img.shields.io/pypi/v/pybevy)](https://pypi.org/project/pybevy/)
[![pypi downloads](https://img.shields.io/pypi/dm/pybevy)](https://pypi.org/project/pybevy/)
[![Discord](https://img.shields.io/discord/1282043096002986034.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/hA4zUneA8f)

> **[pybevy.com](https://pybevy.com)**: **Beta.** Python 3.12+. API evolving, breaking changes expected.
> Independently developed, community-maintained. Not affiliated with the Bevy project.

Write Python, save the file, and see your 3D scene update. Use NumPy, JAX, and PyTorch in the same process as a real 3D renderer. A native MCP server puts coding agents on the same loop.

- **Fast hot reload**: edit code, see changes near instantly, no restart, no recompile
- **Built on Bevy's renderer and ECS**: PBR, volumetric fog, cascaded shadows, bloom, SSAO, and more
- **Python ecosystem in-process**: NumPy, JAX, PyTorch, Numba. Just import.
- **Native MCP server**: fast iteration and hot reload for coding agents
- **If you know Bevy's Rust API**, PyBevy should feel immediately familiar

## Getting Started

### Installation

```bash
$ pip install pybevy
```

Pre-compiled wheels are available for Linux (x86_64), macOS (Apple Silicon and x86_64), and Windows (x86_64). Python 3.12+. See [Installation](docs/installation.md) for further details.

PyBevy ships an integrated [MCP server](https://modelcontextprotocol.io/), so a coding agent works the same loop you do: write Python, hot reload, screenshot the result, inspect entities. See [docs/mcp.md](docs/mcp.md) for full setup.

```bash
codex mcp add pybevy -- pybevy mcp    # or claude/gemini/opencode...
```

## Quick Example

Parent-child entity hierarchy with a rotating parent cube. The child cube inherits the parent's transform automatically.

```python
from pybevy.decorators import component, entrypoint
from pybevy.prelude import *


@component
class Rotator(Component):
    """Marks entities that should rotate."""


def rotate(time: Res[Time], query: Query[Mut[Transform], With[Rotator]]) -> None:
    for transform in query:
        transform.rotate_x(3.0 * time.delta_secs())


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    cube = meshes.add(Cuboid(2.0, 2.0, 2.0))
    material = materials.add(StandardMaterial(base_color=Color.srgb(0.8, 0.7, 0.6)))

    commands.spawn(
        Mesh3d(cube), MeshMaterial3d(material),
        Transform.from_xyz(0.0, 0.0, 1.0), Rotator(),
    ).with_children(
        lambda parent: parent.spawn(
            Mesh3d(cube), MeshMaterial3d(material),
            Transform.from_xyz(0.0, 0.0, 3.0),
        )
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
        .add_systems(Update, rotate)
    )


if __name__ == "__main__":
    main().run()
```

Save this as `main.py` and run with hot reload. Edit the code. The engine hot reloads near instantly, no restart, no recompile.

```bash
pybevy watch main.py
```


### Integrate with Rust-based Bevy

If you already have a Rust Bevy application, you can embed Python systems into it with `PyBevyPlugin`. Prototype in Python, ship critical paths in Rust.

> **Security:** `PyBevyPlugin` embeds a full CPython interpreter with unrestricted host access. Never execute untrusted Python code. We advise against using this for user-submitted plugin/modding systems without external sandboxing.

```toml
# Cargo.toml
[dependencies]
bevy = "0.19"
pybevy = { version = "0.3", features = ["native-plugin"] }
```

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

Reflected world data can be exchanged with Rust Bevy through `.scn.ron` assets. See the [Bevy interoperability guide](docs/bevy-interop.md).

## Bevy Compatibility

PyBevy versions target specific Bevy versions:

| pybevy | Bevy |
| ------ | ---- |
| 0.3.x  | 0.19 |
| 0.2.x  | 0.18 |
| 0.1.x  | 0.18 |

PyBevy follows Bevy's API conventions as closely as possible and targets full coverage of Bevy's public API. Core modules like transforms, lighting, cameras, and input are fully covered; others are in progress. See [Limitations](docs/limitations.md) for known constraints.

## Development Process

PyBevy started in May 2025 as a pure-Python ECS prototype, then moved through a
ctypes FFI layer before pivoting to PyO3, which became the real foundation.
Hand-written `.pyi` type stubs drove the API design before the Rust integration was
fully in place. The project also builds on Bevy experience going back to 2021.

The ECS query model, safety/validity system, and core Bevy component bindings were
developed manually across multiple iterations.

From November 2025 onward, AI tools were used more heavily for API coverage expansion
across Bevy's large surface area, crate splitting into ~30 feature crates, and parts
of the test/stub/documentation workflow.

To keep that process grounded, PyBevy is backed by a custom Rust API compliance tool
that validates bindings against Bevy's source and the Python stubs, and an unpublished
test suite spanning 100K+ lines.

## Limitations

- **No built-in physics**: use NumPy or JAX for physics computation, PyBevy for visualization.
- **Desktop only**: Linux, macOS, Windows. No mobile.
- **Code only**: no visual editor.
- **API is evolving**: see [Limitations](docs/limitations.md) for known constraints.

## Documentation

- **[pybevy.com](https://pybevy.com)**: Project website
- **[Examples](https://github.com/pybevy/pybevy/tree/main/examples)**: Runnable examples covering 2D, 3D, ECS, animation, and more
- **[Bevy interoperability](docs/bevy-interop.md)**: Exchange reflected world data through Bevy `.scn.ron` assets
- **[Limitations](docs/limitations.md)**: Known limitations

## Community & Contributing

- **Discord:** [Pybevy Discord](https://discord.gg/hA4zUneA8f), questions, discussion, showcases
- **Contributing:** See [CONTRIBUTING.md](https://github.com/pybevy/pybevy/blob/main/CONTRIBUTING.md)

## License

All code in this repository is dual-licensed under either:

- [MIT License](https://github.com/pybevy/pybevy/blob/main/LICENSE-MIT)
- [Apache License 2.0](https://github.com/pybevy/pybevy/blob/main/LICENSE-APACHE)

at your option.

By contributing, you agree your work will be released under both licenses.
