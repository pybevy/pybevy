"""Illustrates how to scale an object in each direction.

Demonstrates:
- Scale manipulation via Transform.scale
- Cycling through scale directions (X, Y, Z)
- Min/max bounds checking
- Vector operations (max_element, min_element, zxy permutation)

The cube scales up and down, cycling through X, Y, and Z axes.
"""

import math

from pybevy.assets import Assets
from pybevy.ecs import Commands, Query, Res, ResMut
from pybevy.math import Cuboid, Quat, Vec3
from pybevy.mesh import Mesh3d, MeshMaterial3d
from pybevy.prelude import *
from pybevy.render import StandardMaterial


@component
class Scaling(Component):
    """Marker component for scaling entities."""


@resource
class ScalingSettings(Resource):
    """Scaling parameters stored as a resource."""
    def __init__(self):
        self.scale_direction = Vec3.X
        self.scale_speed = 2.0
        self.max_element_size = 5.0
        self.min_element_size = 1.0


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up the scene with a scaling cube."""
    # Spawn a cube to scale (rotated for better visibility)
    commands.spawn(
        Mesh3d(meshes.add(Cuboid().mesh())),
        MeshMaterial3d(materials.add(StandardMaterial(base_color=Color.WHITE))),
        Transform.from_rotation(Quat.from_rotation_y(math.pi / 4.0)),
        Scaling(),
    )

    # Camera
    commands.spawn(Camera3d(), Transform.from_xyz(0.0, 10.0, 20.0).looking_at(Vec3.ZERO, Vec3.Y))

    # Light source
    commands.spawn(DirectionalLight(), Transform.from_xyz(3.0, 3.0, 3.0).looking_at(Vec3.ZERO, Vec3.Y))


def change_scale_direction(
    cubes: Query[Mut[Transform], With[Scaling]],
    settings: ResMut[ScalingSettings],
) -> None:
    """Check bounds and change scaling direction when limits reached."""
    for transform in cubes:
        # If scaled beyond maximum, flip direction and floor to max
        if transform.scale.max_element() > settings.max_element_size:
            settings.scale_direction *= -1.0
            transform.scale = transform.scale.floor()

        # If scaled beyond minimum, flip direction, ceil to min, and cycle axis
        if transform.scale.min_element() < settings.min_element_size:
            settings.scale_direction *= -1.0
            transform.scale = transform.scale.ceil()
            # Cycle through dimensions: X -> Y -> Z -> X
            settings.scale_direction = settings.scale_direction.zxy()


def scale_cube(
    cubes: Query[Mut[Transform], With[Scaling]],
    settings: Res[ScalingSettings],
    time: Res[Time],
) -> None:
    """Scale entities with Scaling component."""
    for transform in cubes:
        transform.scale += settings.scale_direction * settings.scale_speed * time.delta_secs()


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(ScalingSettings())
        .add_systems(Startup, setup)
        .add_systems(Update, (change_scale_direction, scale_cube))
    )


if __name__ == "__main__":
    main().run()
