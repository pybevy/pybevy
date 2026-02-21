# pybevy: Python 2D/3D Powered by the Bevy Engine

[![Discord](https://img.shields.io/discord/1282043096002986034.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/hA4zUneA8f)
[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/pybevy/pybevy#license)
[![pypi](https://img.shields.io/pypi/v/pybevy)](https://pypi.org/project/pybevy/)
[![pypi downloads](https://img.shields.io/pypi/dm/pybevy)](https://pypi.org/project/pybevy/)

**[pybevy.com](https://pybevy.com)** — Project website & documentation

> **Warning: Beta State**
>
> Pybevy is in an early and experimental stage. The API is incomplete, subject to breaking changes without notice, and you should expect bugs. Many features are still under development.

## What is pybevy?

**Pybevy** is a Python extension that bridges the productivity of Python with the performance of the [Bevy Engine](https://bevy.org/), a refreshingly simple data-driven game engine built in Rust.

**Hot-reload is the star feature** - modify your Python code and see changes instantly in your running application, making development incredibly fast and iterative. **Combine the vast Python ecosystem** with Bevy's cutting-edge 3D engine: leverage NumPy, SciPy, Matplotlib, PyTorch, and thousands of other packages while enjoying high-performance 3D rendering and physics. Write clean, Pythonic code to build games, simulations, and interactive applications without sacrificing speed. pybevy empowers you to leverage Bevy's massively parallel, cache-friendly ECS (Entity Component System) architecture directly from Python.

![Hot reloading demo](https://raw.githubusercontent.com/pybevy/pybevy/main/assets/demo.webp)

`pybevy` is an independent, community-driven project built with great respect for the Bevy Engine and its developers. It is not officially affiliated with the Bevy project.

## Why `pybevy`?

`pybevy` is designed for developers who love Python but need more performance for graphically-intensive applications.

- **Lightning-Fast Development with Hot-Reload:** See your code changes instantly without restarting your application. Modify game logic, adjust parameters, or tweak visuals and watch them update in real-time, dramatically accelerating your development workflow.
- **Python Ecosystem Meets Cutting-Edge 3D:** Access the entire Python ecosystem—NumPy, Pandas, SciPy, PyTorch, OpenCV, and more—while rendering with Bevy's state-of-the-art 3D engine. Build data-driven applications, AI-powered games, or scientific visualizations with the best of both worlds.
- **Performance Without Compromise:** Eliminate the "performance wall" common in Python 3D development. Performance-critical engine internals—like rendering and scheduling—run at near-native speed in Rust, while your game logic lives in expressive Python.
- **Rapid, Scalable Prototyping:** Start your project entirely in Python to iterate quickly. As your needs grow, seamlessly rewrite performance-critical systems in Rust for a powerful hybrid approach. Your prototype can become your final product without switching engines.
- **High-Performance Visualization:** The perfect backend for visualizing massive datasets (e.g., point clouds, simulations) in real-time by leveraging Bevy's rendering power with data loaded via libraries like NumPy and Open3D.
- **Modern ECS Architecture, Made Accessible:** Learn and use Bevy's powerful, data-oriented Entity Component System paradigm with simple Python functions and classes. It's an ideal tool for teaching modern engine design without the steep learning curve of Rust.

## Getting Started

### Installation

Requires **Python 3.12+**. Pre-compiled wheels are available for the following platforms:

| Platform | Architecture |
|----------|-------------|
| Linux    | x86_64      |
| macOS    | ARM (Apple Silicon), x86_64 |
| Windows  | x86_64      |

```bash
pip install pybevy --upgrade
```

## Quick Look: The `pybevy` Syntax

The API is designed to feel natural and intuitive, closely mirroring Bevy's own "refreshingly simple" philosophy.

### Pure Python Approach

A simple 3D scene, written entirely in Python. Perfect for getting started and rapid iteration.

```python
# A simple 3D scene with a cube sitting on a plane.
from pybevy.prelude import *
import math

# Bevy systems are just Python functions with type-hinted parameters.
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
        Mesh3d(meshes.add(Cuboid(1.0, 1.0, 1.0).mesh())),
        MeshMaterial3d(materials.add(StandardMaterial.from_color(Color.srgb_u8(124, 144, 255)))),
        Transform.from_xyz(0.0, 0.5, 0.0),
    )

    # Light
    commands.spawn(
        PointLight(shadows_enabled=True),
        Transform.from_xyz(4.0, 8.0, 4.0),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3.ZERO, Vec3.Y),
    )

@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)

if __name__ == "__main__":
    main().run()
```

Save this as `main.py` and run it with the `pybevy watch` command, which starts your app in hot-reloading mode:

```bash
pybevy watch --full main.py
```

Now you can edit `main.py` while the application is running, and see your changes reflected instantly!

The `--full` flag reloads everything on each change, including setup systems (scene creation, spawning entities). Without it, only game logic (Update systems) is reloaded — faster for iterating on runtime behavior once your scene is set up.

## Bevy Compatibility

`pybevy` versions target specific Bevy versions:

| pybevy  | Bevy |
|---------|------|
| 0.1.x   | 0.18 |

## Documentation

- **[pybevy.com](https://pybevy.com)** — Project website
- **[Examples](https://github.com/pybevy/pybevy/tree/main/examples)** — Runnable examples covering 2D, 3D, ECS, animation, physics, and more

## Community & Contributing

- **Join the Conversation:** The best place to ask questions and chat is the official [Pybevy Discord](https://discord.gg/hA4zUneA8f).
- **Contributing:** See [CONTRIBUTING.md](https://github.com/pybevy/pybevy/blob/main/CONTRIBUTING.md) for how to get involved.

## License

All code in this repository is dual-licensed under either:

- [MIT License](https://github.com/pybevy/pybevy/blob/main/LICENSE-MIT)
- [Apache License 2.0](https://github.com/pybevy/pybevy/blob/main/LICENSE-APACHE)

at your option.

By contributing, you agree your work will be released under both licenses.
