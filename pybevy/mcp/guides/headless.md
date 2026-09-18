# Headless Rendering Guide

GPU rendering without a window or display server - for CI, remote servers, containers, and automated testing.

`Image.new_render_target` creates an RGBA8 sRGB target with `TEXTURE_BINDING`,
`COPY_SRC`, `COPY_DST`, and `RENDER_ATTACHMENT`. It is a convenience for
`Image.new_target_texture` with that format and `COPY_SRC` added. Import
`TextureUsages` from `pybevy.render`; `insert`, `remove`, `toggle`, and `set`
mutate a flag value in place. `Image.texture_descriptor.usage` borrows asset flags; nested writes
require `Assets[Image].get_mut`. For an owned image, copy its descriptor with
`copy.copy()`, modify the copy's `usage`, then assign the descriptor back.
`CameraMainTextureUsages(value=TextureUsages.COPY_SRC)` uses the same flags
type; its `value` property and `with_` builder use `TextureUsages`.

## When to Use Headless

- No display server available (SSH, CI runners, Docker containers)
- Automated screenshot pipelines
- Server-side rendering
- Integration tests

## Loading Image Files

Creating `Image` values in memory only needs `AssetPlugin` and `ImagePlugin`
with `MinimalPlugins`. Loading PNG or JPEG files also needs `TexturePlugin`,
included in `RenderPlugin`. Adding `TexturePlugin` alone reports a missing
`RenderApp`; PyBevy exposes no renderer-free file-image loader. Use this stack:

```python
from pybevy.app import MinimalPlugins, ScheduleRunnerPlugin
from pybevy.assets import AssetPlugin
from pybevy.camera import CameraPlugin
from pybevy.image import ImagePlugin
from pybevy.mesh import MeshPlugin
from pybevy.render import RenderPlugin

app.add_plugins(
    MinimalPlugins,
    AssetPlugin,
    RenderPlugin,  # Includes TexturePlugin and creates the headless RenderApp.
    ImagePlugin,
    MeshPlugin,    # Satisfies RenderPlugin's mesh extraction systems.
    CameraPlugin,  # Provides ClearColor for RenderPlugin's view systems.
    ScheduleRunnerPlugin.run_loop(16),
)
```

## Minimal Working Example

```python
from pybevy.prelude import *
from pybevy.app import ScheduleRunnerPlugin
from pybevy.camera import ImageRenderTarget, RenderTarget, ShadowLodOrigin
from pybevy.image import Image
from pybevy.ui import IsDefaultUiCamera
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
        ShadowLodOrigin(),
        IsDefaultUiCamera(),
        RenderTarget.Image(ImageRenderTarget(handle=handle)),
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
| Camera target | Screen (automatic) | `RenderTarget.Image(ImageRenderTarget(handle=handle))` (explicit offscreen) |
| UI camera | Primary camera (automatic) | Add `IsDefaultUiCamera()` to the offscreen camera |
| WinitPlugin | Enabled | `.disable(WinitPlugin)` |

## Three Required Changes

1. **Disable WinitPlugin** - it requires a display server:
   ```python
   (
       DefaultPlugins()
       .set(WindowPlugin(primary_window=None, exit_condition=ExitCondition.DontExit))
       .disable(WinitPlugin)
   )
   ```

2. **Add ScheduleRunnerPlugin** - provides the frame loop without a window:
   ```python
   app.add_plugins(ScheduleRunnerPlugin.run_loop(16))  # 16ms ≈ 60fps
   ```

3. **Use offscreen render target** - camera must render to an image, not the screen:
   ```python
   render_target = Image.new_render_target(width=256, height=256)
   handle = images.add(render_target)
   commands.spawn(Camera3d(), Camera(), RenderTarget.Image(ImageRenderTarget(handle=handle)), transform)
   ```

   If the scene has UI `Node` or `Text` entities, also add
   `IsDefaultUiCamera()` to this camera. With no primary window, Bevy cannot
   choose a UI camera automatically. World-space `Text2d` does not require it.

## MCP Usage

Launch headless scenes with the `headless=True` parameter:

```
run_scene(path="my_scene.py", headless=True)
```

`run_scene` returns after the control server responds and first-frame system
errors can be checked, with a 60-second startup deadline. Asset loads and GPU
readback frames may complete later. A startup timeout stops the subprocess and
includes its captured output in the error.

All MCP tools work in headless mode:
- `capture_screenshot`, `capture_stats`, `capture_turnaround`, `capture_depth` use GPU readback
- `set_component`, `spawn_entity`, `query_entities` work normally
- `reload`, `reload_and_capture` work normally

## Troubleshooting

- **"No display server" error**: Make sure `WinitPlugin` is disabled and `headless=True` is passed to `run_scene`
- **Black screenshots**: Ensure the camera has `RenderTarget.Image(ImageRenderTarget(handle=handle))` - without it, the camera targets a non-existent window
- **No frames captured**: Increase `delay_frames` in `capture_screenshot` - headless rendering may need more warmup frames
- **UI is missing**: Add `IsDefaultUiCamera()` to the offscreen camera. MCP
  captures hide authored UI by default, so also pass `hide_ui=false` to
  `capture_screenshot`, `capture_stats`, `capture_turnaround`, or
  `capture_timeline` when the UI should be visible.
- **Shadow LOD warning**: Add `ShadowLodOrigin()` to an offscreen camera when using point or spot light shadows
- **Low resolution**: The render target size (`width`, `height` in `Image.new_render_target`) determines output resolution, not window size

Image readback returns one `width * height` frame at the size the readback was requested with. For an array-texture source it copies layer zero only and logs a warning once; additional layers are not concatenated into the frame. If the render target is later resized, the copy stays inside the frame that was requested instead of overrunning it.
