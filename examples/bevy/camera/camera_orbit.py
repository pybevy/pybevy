"""Camera orbit around a 3D scene using pitch, yaw, and roll.

Demonstrates:
- 3D camera orbit controls
- Mouse input (motion and buttons)
- Euler angle rotations (YXZ order)
- 3D primitives (Plane, Cuboid)
- Camera Transform manipulation

Controls:
- Mouse up/down: Pitch (rotate around X axis)
- Mouse left/right: Yaw (rotate around Y axis)
- Left mouse button: Roll counter-clockwise
- Right mouse button: Roll clockwise
"""

import math

from pybevy.input import AccumulatedMouseMotion, ButtonInput, MouseButton
from pybevy.prelude import *


@resource
class CameraSettings(Resource):
    """Settings for camera orbit behavior."""

    def __init__(self):
        # Limiting pitch stops unexpected rotation past 90° up or down
        pitch_limit = math.pi / 2 - 0.01

        self.orbit_distance = 20.0
        self.pitch_speed = 0.003
        self.pitch_range_start = -pitch_limit
        self.pitch_range_end = pitch_limit
        self.roll_speed = 1.0
        self.yaw_speed = 0.004


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up a simple 3D scene."""
    # Camera
    camera_transform = Transform.from_xyz(5.0, 5.0, 5.0).looking_at(Vec3.ZERO, Vec3.Y)
    commands.spawn(
        Camera3d(),
        camera_transform,
    )

    # Plane
    plane_mesh = Plane3d().mesh().size(5.0, 5.0)
    plane_material = StandardMaterial(
        base_color=Color.srgb(0.3, 0.5, 0.3),
        cull_mode=None,  # Keep visible when viewed from beneath
    )
    commands.spawn(
        Mesh3d(meshes.add(plane_mesh)),
        MeshMaterial3d(materials.add(plane_material)),
    )

    # Cube
    cube_mesh = Cuboid().mesh()
    cube_material = StandardMaterial(base_color=Color.srgb(0.8, 0.7, 0.6))
    commands.spawn(
        Mesh3d(meshes.add(cube_mesh)),
        MeshMaterial3d(materials.add(cube_material)),
        Transform.from_xyz(1.5, 0.51, 1.5),
    )

    # Light
    commands.spawn(
        PointLight(),
        Transform.from_xyz(3.0, 8.0, 5.0),
    )


def orbit(
    camera_query: Single[Mut[Transform], With[Camera3d]],
    camera_settings: Res[CameraSettings],
    mouse_buttons: Res[ButtonInput[MouseButton]],
    mouse_motion: Res[AccumulatedMouseMotion],
    time: Res[Time],
) -> None:
    """Orbit camera based on mouse input."""
    for camera in camera_query:
        delta = Vec2(mouse_motion.delta.x, mouse_motion.delta.y)
        delta_roll = 0.0

        if mouse_buttons.pressed(MouseButton.Left()):
            delta_roll -= 1.0

        if mouse_buttons.pressed(MouseButton.Right()):
            delta_roll += 1.0

        # Mouse motion should not be multiplied by delta time as we already
        # receive the full movement since the last frame was rendered
        delta_pitch = delta.y * camera_settings.pitch_speed
        delta_yaw = delta.x * camera_settings.yaw_speed

        # DO factor in delta time for mouse button inputs
        delta_roll *= camera_settings.roll_speed * time.delta_secs()

        # Obtain existing pitch, yaw, roll from the transform rotation
        yaw, pitch, roll = camera.rotation.to_euler(EulerRot.YXZ)

        # Establish new yaw and pitch, clamping pitch to limits
        pitch = max(camera_settings.pitch_range_start,
                   min(pitch + delta_pitch, camera_settings.pitch_range_end))
        roll = roll + delta_roll
        yaw = yaw + delta_yaw

        camera.rotation = Quat.from_euler(EulerRot.YXZ, yaw, pitch, roll)

        # Adjust translation to maintain correct orientation toward orbit target
        # In this example it's a static target, but could be customized
        target = Vec3.ZERO
        camera.translation = target - camera.forward() * camera_settings.orbit_distance


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(CameraSettings())
        .add_systems(Startup, setup)
        .add_systems(Update, orbit)
    )


if __name__ == "__main__":
    main().run()
