"""Illustrates how to move an object along an axis.

Demonstrates:
- Translation using Transform.translation
- Custom component storing movement parameters
- Bouncing movement using distance checks
- local_x() direction vector

The cube moves back and forth along its local X axis.
"""

from pybevy.assets import Assets
from pybevy.ecs import Commands, Query, Res, ResMut
from pybevy.math import Cuboid, Vec3
from pybevy.mesh import Mesh3d, MeshMaterial3d
from pybevy.prelude import *
from pybevy.pbr import StandardMaterial


@component
class Movable(Component):
    """Marker component for movable entities."""


@resource
class MovementSettings(Resource):
    """Movement parameters stored as a resource."""
    def __init__(self):
        self.spawn = Vec3.ZERO
        self.max_distance = 5.0
        self.speed = 2.0


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up the scene with a moving cube."""
    entity_spawn = Vec3.ZERO

    # Add a cube to visualize translation
    commands.spawn(
        Mesh3d(meshes.add(Cuboid().mesh())),
        MeshMaterial3d(materials.add(StandardMaterial(base_color=Color.WHITE))),
        Transform.from_translation(entity_spawn),
        Movable(),
    )

    # Camera
    commands.spawn(Camera3d(), Transform.from_xyz(0.0, 10.0, 20.0).looking_at(entity_spawn, Vec3.Y))

    # Light source
    commands.spawn(DirectionalLight(), Transform.from_xyz(3.0, 3.0, 3.0).looking_at(Vec3.ZERO, Vec3.Y))


def move_cube(
    cubes: Query[Mut[Transform], With[Movable]],
    settings: ResMut[MovementSettings],
    time: Res[Time],
) -> None:
    """Move entities with Movable component back and forth."""
    for transform in cubes:
        # Check if entity moved too far from spawn, if so reverse direction
        distance = (settings.spawn - transform.translation).length()
        if distance > settings.max_distance:
            settings.speed *= -1.0

        # Move along local X axis
        direction = transform.local_x()
        transform.translation += direction * settings.speed * time.delta_secs()


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(MovementSettings())
        .add_systems(Startup, setup)
        .add_systems(Update, move_cube)
    )


if __name__ == "__main__":
    main().run()
