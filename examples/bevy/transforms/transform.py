"""Simplified transform demonstration.

Shows translation, rotation, and scaling of 3D objects. This is a simplified
version of Bevy's transform.rs example, adapted to work within PyBevy's current
limitations with disjoint queries.

Demonstrates:
- A cube that orbits around a central point
- Smooth rotation using quaternion interpolation
- Dynamic scaling based on distance traveled
"""

import math
from dataclasses import dataclass

from pybevy.prelude import *


@component
@dataclass
class Orbiter(Component):
    """Marks an entity that orbits around the origin."""
    move_speed: float = 2.0
    turn_speed: float = 0.2


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up the scene."""
    # Central sphere (will scale down over time)
    commands.spawn(
        Mesh3d(meshes.add(Sphere(radius=1.0).mesh().ico(32))),
        MeshMaterial3d(materials.add(StandardMaterial(base_color=Color.srgb(1.0, 1.0, 0.0)))),
        Transform.from_translation(Vec3.ZERO),
    )

    # Orbiting cube
    cube_spawn = Transform.from_translation(Vec3(0.0, 0.0, -10.0)).with_rotation(
        Quat.from_rotation_y(math.pi / 2.0)
    )
    commands.spawn(
        Mesh3d(meshes.add(Cuboid().mesh())),
        MeshMaterial3d(materials.add(StandardMaterial(base_color=Color.WHITE))),
        cube_spawn,
        Orbiter(),
    )

    # Camera
    commands.spawn(Camera3d(), Transform.from_xyz(0.0, 10.0, 20.0).looking_at(Vec3.ZERO, Vec3.Y))

    # Light
    commands.spawn(DirectionalLight(), Transform.from_xyz(3.0, 3.0, 3.0).looking_at(Vec3.ZERO, Vec3.Y))


def move_and_rotate_orbiter(
    orbiters: Query[tuple[Mut[Transform], Orbiter]],
    time: Res[Time],
) -> None:
    """Move the orbiting cube forward and rotate it towards the center."""
    for transform, orbiter in orbiters:
        # Move forward
        forward = transform.forward()
        transform.translation = (
            transform.translation + forward * orbiter.move_speed * time.delta_secs()
        )

        # Rotate towards center
        look_at_center = transform.looking_at(Vec3.ZERO, transform.local_y())
        incremental_turn = orbiter.turn_speed * time.delta_secs()
        transform.rotation = transform.rotation.lerp(look_at_center.rotation, incremental_turn)


@entrypoint
def main(app: App) -> App:
    """Configure and return the app."""
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, move_and_rotate_orbiter)
    )


if __name__ == "__main__":
    main().run()
