# Headless Rendering Guide

GPU rendering without a window or display server - for CI, remote servers, containers, and automated testing.

## When to Use Headless

- No display server available (SSH, CI runners, Docker containers)
- Automated screenshot pipelines
- Server-side rendering
- Integration tests

## Minimal Working Example

```python
from pybevy.prelude import *
from pybevy.app import ScheduleRunnerPlugin
from pybevy.camera import RenderTarget
from pybevy.image import Image
from pybevy.window import ExitCondition, WindowPlugin
from pybevy.winit import WinitPlugin


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
    images: ResMut[Assets[Image]],
) -> None:
    # Scene content
    commands.spawn(
        Mesh3d(meshes.add(Cuboid(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial.from_color(Color.srgb(0.5, 0.6, 1.0)))),
        Transform.from_xyz(0.0, 0.5, 0.0),
    )
    commands.spawn(PointLight(shadow_maps_enabled=True), Transform.from_xyz(4.0, 8.0, 4.0))

    # Camera rendering to offscreen image (required for headless)
    render_target = Image.new_render_target(width=256, height=256)
    handle = images.add(render_target)
    commands.spawn(
        Camera3d(),
        Camera(),
        RenderTarget.image(handle),
        Transform.from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3.ZERO, Vec3.Y),
    )


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(
            DefaultPlugins()
            .set(WindowPlugin(primary_window=None, exit_condition=ExitCondition.DontExit))
            .disable(WinitPlugin)
        )
        .add_plugins(ScheduleRunnerPlugin.run_loop(16))
        .add_systems(Startup, setup)
    )


if __name__ == "__main__":
    main().run()
```

## Key Differences from Windowed Scenes

| Setting | Windowed (default) | Headless |
|---------|-------------------|----------|
| Window | `WindowPlugin` default | `WindowPlugin(primary_window=None, exit_condition=ExitCondition.DontExit)` |
| Event loop | WinitPlugin (display-driven) | `ScheduleRunnerPlugin.run_loop(16)` (timer-driven, ~60fps) |
| Camera target | Screen (automatic) | `RenderTarget.image(handle)` (explicit offscreen) |
| WinitPlugin | Enabled | `.disable(WinitPlugin)` |

## Three Required Changes

1. **Disable WinitPlugin** - it requires a display server:
   ```python
   DefaultPlugins()
       .set(WindowPlugin(primary_window=None, exit_condition=ExitCondition.DontExit))
       .disable(WinitPlugin)
   ```

2. **Add ScheduleRunnerPlugin** - provides the frame loop without a window:
   ```python
   app.add_plugins(ScheduleRunnerPlugin.run_loop(16))  # 16ms ≈ 60fps
   ```

3. **Use offscreen render target** - camera must render to an image, not the screen:
   ```python
   render_target = Image.new_render_target(width=256, height=256)
   handle = images.add(render_target)
   commands.spawn(Camera3d(), Camera(), RenderTarget.image(handle), transform)
   ```

## MCP Usage

Launch headless scenes with the `headless=True` parameter:

```
run_scene(path="scenes/my_scene.py", headless=True)
```

All MCP tools work in headless mode:
- `capture_screenshot`, `capture_turnaround`, `capture_depth` use GPU readback
- `set_component`, `spawn_entity`, `query_entities` work normally
- `reload`, `reload_and_capture` work normally

## Troubleshooting

- **"No display server" error**: Make sure `WinitPlugin` is disabled and `headless=True` is passed to `run_scene`
- **Black screenshots**: Ensure the camera has `RenderTarget.image(handle)` - without it, the camera targets a non-existent window
- **No frames captured**: Increase `delay_frames` in `capture_screenshot` - headless rendering may need more warmup frames
- **Low resolution**: The render target size (`width`, `height` in `Image.new_render_target`) determines output resolution, not window size

## Reference Example

See `examples/misc/headless_render.py` for a complete working example with a rotating cube.
