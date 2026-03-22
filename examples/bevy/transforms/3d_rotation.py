"""Illustrates how to rotate an object around an axis.

Demonstrates:
- Rotation using Transform.rotate_y()
- Custom component with speed parameter
- Time-based rotation (rotations per second)

The Rotatable component stores rotation speed in rotations per second.
"""

import math
from dataclasses import dataclass

from pybevy.assets import Assets
from pybevy.ecs import Commands, Query, Res, ResMut
from pybevy.math import Cuboid, Vec3
from pybevy.mesh import Mesh3d, MeshMaterial3d
from pybevy.prelude import *
from pybevy.render import StandardMaterial


@component
@dataclass
class Rotatable(Component):
    """Component to designate a rotation speed to an entity."""
    speed: float = 0.0


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up the scene with a rotating cube."""
    # Spawn a cube to rotate
    commands.spawn(
        Mesh3d(meshes.add(Cuboid().mesh())),
        MeshMaterial3d(materials.add(StandardMaterial(base_color=Color.WHITE))),
        Transform.from_translation(Vec3.ZERO),
        Rotatable(speed=0.3),
    )

    # Camera
    commands.spawn(Camera3d(), Transform.from_xyz(0.0, 10.0, 20.0).looking_at(Vec3.ZERO, Vec3.Y))

    # Light source
    commands.spawn(DirectionalLight(), Transform.from_xyz(3.0, 3.0, 3.0).looking_at(Vec3.ZERO, Vec3.Y))


def rotate_cube(cubes: Query[tuple[Mut[Transform], Rotatable]], time: Res[Time]) -> None:
    """Rotate entities with Rotatable component around their y-axis."""
    for transform, rotatable in cubes:
        # Speed is multiplied by TAU (full rotation in radians)
        # then by delta_secs to get smooth rotation per second
        transform.rotate_y(rotatable.speed * math.tau * time.delta_secs())


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, rotate_cube)
    )


if __name__ == "__main__":
    main().run()
