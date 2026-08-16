# Crates

PyBevy is split into modular feature crates.

## Core

| Crate | Description |
|-------|-------------|
| [`pybevy_core`](pybevy_core/) | Base classes (PyComponent, PyResource, PyAsset, PyPlugin), storage types, registry |
| [`pybevy_ecs`](pybevy_ecs/) | ECS module: queries, views, commands, systems, components, resources |
| [`pybevy_macros`](pybevy_macros/) | Proc macros (pycomponent, bridge macros, pyenum, etc.) |
| [`pybevy_python`](pybevy_python/) | cdylib entry point (`_pybevy` module) |
| [`pybevy_bytecodevm`](pybevy_bytecodevm/) | Bytecode VM for lazy View expressions |
| [`pybevy_storage`](pybevy_storage/) | PyO3-free storage primitives (ComponentStorage, ValueStorage, etc.) |

## Tools

| Crate | Description |
|-------|-------------|
| [`pybevy_control`](pybevy_control/) | HTTP control API for remote interaction with running scenes |
| [`pybevy_lint`](pybevy_lint/) | API coverage linting tool |

## Bevy Module Wrappers

| Crate | Wraps |
|-------|-------|
| [`pybevy_a11y`](pybevy_a11y/) | Accessibility |
| [`pybevy_animation`](pybevy_animation/) | Animation clips, graphs, players |
| [`pybevy_audio`](pybevy_audio/) | Audio sources, sinks, spatial audio |
| [`pybevy_camera`](pybevy_camera/) | Camera2d/3d, bloom, color grading, projection, tonemapping |
| [`pybevy_color`](pybevy_color/) | Color types (LinearRgba, Srgba, etc.) |
| [`pybevy_gltf`](pybevy_gltf/) | GLTF loading |
| [`pybevy_image`](pybevy_image/) | Image, TextureAtlas, TextureAtlasLayout |
| [`pybevy_input`](pybevy_input/) | Keyboard, mouse, gamepad, touch input |
| [`pybevy_light`](pybevy_light/) | Point/Directional/Spot lights, shadows, fog |
| [`pybevy_math`](pybevy_math/) | Vec2/3/4, Quat, Mat3/4, primitives |
| [`pybevy_mesh`](pybevy_mesh/) | Mesh, mesh builders, primitives with Meshable |
| [`pybevy_pbr`](pybevy_pbr/) | PBR materials, wireframe, fog, decals |
| [`pybevy_render`](pybevy_render/) | Render pipeline, shaders, visibility |
| [`pybevy_shader`](pybevy_shader/) | Shader type wrappers (Shader, ShaderRef, ShaderImport, etc.) |
| [`pybevy_shader_types`](pybevy_shader_types/) | Shared ShaderMaterial types (PyO3-free, for plugin/WASM) |
| [`pybevy_sprite`](pybevy_sprite/) | Sprite, SpriteSheet |
| [`pybevy_text`](pybevy_text/) | Text rendering |
| [`pybevy_time`](pybevy_time/) | Time, Timer, Stopwatch |
| [`pybevy_transform`](pybevy_transform/) | Transform, GlobalTransform |
| [`pybevy_ui`](pybevy_ui/) | UI nodes, styles, interaction |
| [`pybevy_window`](pybevy_window/) | Window management, cursor, events |
| [`pybevy_world_serialization`](pybevy_world_serialization/) | World serialization (WorldAsset, DynamicWorld, spawning) |
