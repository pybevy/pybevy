"""Python systems for the native_plugin_example.

Demonstrates:
- Setting up a 3D scene (standard PyBevy)
- Querying and mutating a Rust-defined component (Health) from Python
- Hot reload: edit ROTATION_SPEED below and press F5 to see the change!
"""

import math

# Import the Rust-defined Health component (injected by register_component)
from pybevy._pybevy import Health  # type: ignore[import-not-found]
from pybevy.prelude import *

# Tweak this while the app is running, then press F5!
ROTATION_SPEED = 1.0


@component
class Cube(Component):
    """Marker for the rotating cube."""



def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up a simple 3D scene."""
    # Circular base
    commands.spawn(
        Mesh3d(meshes.add(Circle(radius=4.0).mesh())),
        MeshMaterial3d(materials.add(StandardMaterial.from_color(Color.WHITE))),
        Transform.from_rotation(Quat.from_rotation_x(-math.pi / 2)),
    )

    # Cube
    commands.spawn(
        Cube(),
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


def rotate_cube(query: Query[Mut[Transform], With[Cube]], time: Res[Time]) -> None:
    """Rotate the cube each frame. Change ROTATION_SPEED and reload!"""
    for transform in query:
        transform.rotation *= Quat.from_rotation_y(time.delta_secs() * ROTATION_SPEED)


def apply_damage(query: Query[Mut[Health]], time: Res[Time]) -> None:
    """Python mutates the Rust-defined Health component every frame.

    Rust reads the updated values in its own system and prints them.
    This demonstrates full cross-language component round-trip.
    """
    for health in query:
        health.value -= 5.0 * time.delta_secs()
        if health.value < 0.0:
            health.value = health.max  # Reset when depleted
