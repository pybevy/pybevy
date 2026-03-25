"""Simple 3D scene with light shining over a cube sitting on a plane.

A minimal 3D example demonstrating:
- Basic 3D mesh primitives (Circle, Cuboid)
- StandardMaterial with colors
- PointLight with shadows
- Camera3d with looking_at()
"""

import math

from pybevy.prelude import *


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up a simple 3D scene."""
    # Circular base (rotated to lie flat)
    circle_mesh = Circle(radius=4.0).mesh()
    circle_material = StandardMaterial.from_color(Color.WHITE)
    commands.spawn(
        Mesh3d(meshes.add(circle_mesh)),
        MeshMaterial3d(materials.add(circle_material)),
        Transform.from_rotation(Quat.from_rotation_x(-math.pi / 2)),
    )

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
        PointLight(shadows_enabled=True),
        Transform.from_xyz(4.0, 8.0, 4.0),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3.ZERO, Vec3.Y),
    )


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)


if __name__ == "__main__":
    main().run()
