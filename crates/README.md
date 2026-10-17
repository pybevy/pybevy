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
| [`pybevy_array`](pybevy_array/) | Bounded array core and backend adapters |
| [`pybevy_gpu`](pybevy_gpu/) | GPU tensor contracts, CUDA/Vulkan interop and Python adapters |
| [`pybevy_reload`](pybevy_reload/) | Hot reload orchestration, file watching and cleanup |
| [`pybevy_storage`](pybevy_storage/) | PyO3-free storage primitives (ComponentStorage, ValueStorage, etc.) |

## Tools

| Crate | Description |
|-------|-------------|
| [`pybevy_build`](pybevy_build/) | Build helper that copies PyBevy's Python sources for downstream wheels |
| [`pybevy_control`](pybevy_control/) | HTTP control API for remote interaction with running scenes |
| [`pybevy_lint`](pybevy_lint/) | API coverage linting tool |

## Bevy Module Wrappers

| Crate | Wraps |
|-------|-------|
| [`pybevy_a11y`](pybevy_a11y/) | Accessibility |
| [`pybevy_animation`](pybevy_animation/) | Animation clips, graphs, players |
| [`pybevy_audio`](pybevy_audio/) | Audio sources, sinks, spatial audio |
| [`pybevy_camera`](pybevy_camera/) | Camera2d/3d, projection, viewports, render targets, visibility |
| [`pybevy_color`](pybevy_color/) | Color types (LinearRgba, Srgba, etc.) |
| [`pybevy_core_pipeline`](pybevy_core_pipeline/) | Prepasses |
| [`pybevy_curve`](pybevy_curve/) | Curves, splines, easing, intervals (`pybevy.curve`) |
| [`pybevy_gizmos`](pybevy_gizmos/) | Gizmos |
| [`pybevy_gltf`](pybevy_gltf/) | GLTF loading |
| [`pybevy_image`](pybevy_image/) | Image, TextureAtlas, TextureAtlasLayout |
| [`pybevy_input`](pybevy_input/) | Keyboard, mouse, gamepad, touch input |
| [`pybevy_input_focus`](pybevy_input_focus/) | Input focus and tab navigation |
| [`pybevy_light`](pybevy_light/) | Point/Directional/Spot lights, shadows, fog |
| [`pybevy_material`](pybevy_material/) | Shared material types (AlphaMode, OpaqueRendererMethod) |
| [`pybevy_math`](pybevy_math/) | Vec2/3/4, Quat, Mat3/4, Dir, Rot2, Rect, isometries |
| [`pybevy_mesh`](pybevy_mesh/) | Mesh, mesh builders, `pybevy.shape` primitives with Meshable |
| [`pybevy_pbr`](pybevy_pbr/) | PBR materials, wireframe, fog, decals |
| [`pybevy_post_process`](pybevy_post_process/) | Bloom, depth of field, motion blur, vignette, lens distortion |
| [`pybevy_render`](pybevy_render/) | Textures, formats, color grading, tonemapping, MSAA, readback, shader buffers |
| [`pybevy_shader`](pybevy_shader/) | Shader type wrappers (Shader, ShaderRef, ShaderImport, etc.) |
| [`pybevy_shader_types`](pybevy_shader_types/) | Shared ShaderMaterial types (PyO3-free, for plugin/WASM) |
| [`pybevy_shape`](pybevy_shape/) | Shapes, bounding volumes, rays (`pybevy.shape`) |
| [`pybevy_sprite`](pybevy_sprite/) | Sprite, SpriteSheet |
| [`pybevy_text`](pybevy_text/) | Text rendering |
| [`pybevy_time`](pybevy_time/) | Time, Timer, Stopwatch |
| [`pybevy_transform`](pybevy_transform/) | Transform, GlobalTransform |
| [`pybevy_ui`](pybevy_ui/) | UI nodes, styles, interaction |
| [`pybevy_window`](pybevy_window/) | Window management, cursor, events |
| [`pybevy_winit`](pybevy_winit/) | Winit backend, update mode, winit settings |
| [`pybevy_world_serialization`](pybevy_world_serialization/) | World serialization (WorldAsset, DynamicWorld, spawning) |
