"""Demonstrates 3D shape primitives with basic materials."""

import math

from pybevy.decorators import component, entrypoint
from pybevy.prelude import *


@component
class Shape(Component):
    """Marker component for rotating shapes"""


# Constants for positioning
SHAPES_X_EXTENT = 10.0
Z_EXTENT = 5.0


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
):
    # Create a material with distinct color
    material = materials.add(
        StandardMaterial(
            base_color=Color.srgb(0.8, 0.7, 0.6),
            metallic=0.3,
            perceptual_roughness=0.5,
        )
    )

    # Create different 3D shapes
    shapes = [
        ("Cuboid", meshes.add(Cuboid.from_size(Vec3.splat(1.0)))),
        ("Sphere", meshes.add(Sphere(0.5))),
        ("Cylinder", meshes.add(Cylinder(0.5, 1.0))),
        ("Plane (small)", meshes.add(Plane3d().mesh().size(1.0, 1.0).build())),
    ]

    num_shapes = len(shapes)

    # Spawn each shape in a row
    for i, (_name, mesh_handle) in enumerate(shapes):
        x_pos = -SHAPES_X_EXTENT / 2.0 + (i / (num_shapes - 1)) * SHAPES_X_EXTENT
        commands.spawn(
            Mesh3d(mesh_handle),
            MeshMaterial3d(material),
            Transform.from_xyz(x_pos, 2.0, 0.0).with_rotation(
                Quat.from_rotation_x(-math.pi / 4.0)
            ),
            Shape(),
        )

    # Add a bright point light
    commands.spawn(
        PointLight(
            intensity=10_000_000.0,
            range=100.0,
            shadows_enabled=True,
        ),
        Transform.from_xyz(8.0, 16.0, 8.0),
    )

    # Ground plane
    commands.spawn(
        Mesh3d(meshes.add(Plane3d().mesh().size(50.0, 50.0).build())),
        MeshMaterial3d(
            materials.add(StandardMaterial(base_color=Color.srgb(0.75, 0.75, 0.75)))
        ),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, 7.0, 14.0).looking_at(Vec3(0.0, 1.0, 0.0), Vec3.Y),
    )


def rotate(query: Query[Mut[Transform], With[Shape]], time: Res[Time]):
    """Rotate all shapes"""
    for transform in query:
        transform.rotate_y(time.delta_secs() / 2.0)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, rotate)
    )


if __name__ == "__main__":
    main().run()
