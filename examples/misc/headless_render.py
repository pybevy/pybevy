"""Headless rendering — GPU rendering without a window or display.

Demonstrates:
- Disabling WinitPlugin for environments without a display server
- ScheduleRunnerPlugin as the event loop (replaces winit)
- Camera rendering to an offscreen Image via RenderTarget.image()
- A rotating cube to verify the update loop works

Usage with MCP: run_scene(path=..., headless=True) then capture_screenshot().
"""

from pybevy.app import ScheduleRunnerPlugin
from pybevy.camera import RenderTarget
from pybevy.prelude import *
from pybevy.window import ExitCondition
from pybevy.winit import WinitPlugin


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
    images: ResMut[Assets[Image]],
) -> None:
    """Spawn a cube, light, and camera rendering to an offscreen image."""
    # Cube
    cube_mesh = Cuboid(1.0, 1.0, 1.0).mesh()
    cube_material = StandardMaterial.from_color(Color.srgb_u8(124, 144, 255))
    commands.spawn(
        Mesh3d(meshes.add(cube_mesh)),
        MeshMaterial3d(materials.add(cube_material)),
        Transform.from_xyz(0.0, 0.5, 0.0),
    )

    # Light
    commands.spawn(
        PointLight(shadow_maps_enabled=True),
        Transform.from_xyz(4.0, 8.0, 4.0),
    )

    # Camera rendering to offscreen image
    render_target = Image.new_render_target(width=256, height=256)
    handle = images.add(render_target)
    commands.spawn(
        Camera3d(),
        Camera(),
        RenderTarget.image(handle),
        Transform.from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3.ZERO, Vec3.Y),
    )


def rotate_cube(query: Query[Mut[Transform], With[Mesh3d]], time: Res[Time]) -> None:
    """Rotate the cube around the Y axis."""
    dt = time.delta_secs()
    for transform in query:
        transform.rotation = transform.rotation * Quat.from_rotation_y(dt * 1.0)


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
        .add_systems(Update, rotate_cube)
    )


if __name__ == "__main__":
    main().run()
