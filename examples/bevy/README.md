# Bevy Example Ports

Examples in this folder are PyBevy ports of official Bevy examples from
https://github.com/bevyengine/bevy/tree/v0.19.0/examples.

These examples were automatically translated from Rust to Python by an LLM
and may not be fully accurate or idiomatic. Use them as a starting point.

Each subfolder maps to the corresponding Bevy example category:

| Folder | Bevy source |
|--------|-------------|
| [2d/](2d/) | [examples/2d](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/2d) |
| [3d/](3d/) | [examples/3d](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/3d) |
| [animation/](animation/) | [examples/animation](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/animation) |
| [app/](app/) | [examples/app](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/app) |
| [asset/](asset/) | [examples/asset](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/asset) |
| [audio/](audio/) | [examples/audio](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/audio) |
| [camera/](camera/) | [examples/camera](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/camera) |
| [ecs/](ecs/) | [examples/ecs](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/ecs) |
| [gltf/](gltf/) | [examples/gltf](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/gltf) |
| [input/](input/) | [examples/input](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/input) |
| [large_scenes/](large_scenes/) | [examples/large_scenes](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/large_scenes) |
| [movement/](movement/) | [examples/movement](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/movement) |
| [picking/](picking/) | [examples/picking](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/picking) |
| [stress_tests/](stress_tests/) | [examples/stress_tests](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/stress_tests) |
| [time/](time/) | [examples/time](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/time) |
| [transforms/](transforms/) | [examples/transforms](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/transforms) |
| [window/](window/) | [examples/window](https://github.com/bevyengine/bevy/tree/v0.19.0/examples/window) |

## Storage-buffer material

[shaders/storage_buffer.py](shaders/storage_buffer.py) ports the
[Bevy 0.19.1 storage example](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/shader/storage_buffer.rs).
The material uses `Storage[Vec4]` with generated binding numbers.
It animates a shared read-only vec4 color array and indexes it with `MeshTag`.
The Python annotation selects `StorageShaderMaterialPlugin`; generated bindings
use slot 109 rather than the Rust example's standalone slot 0.

Setup and 60 manually stepped headless frames pass, and the material binding
has separate pixel and reload coverage. **Runtime verification pending:** the
standard example-smoke command needs an X server (`xvfb-run` is unavailable in
this environment). The shared headless scene runner also exits with signal 11
on both this example and the existing `shader_material.py` control; its runner
path remains unqualified. This is not an unqualified completed conversion.
