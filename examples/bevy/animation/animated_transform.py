"""Animates a cube using transform interpolation.

Demonstrates:
- Time-based animation
- Transform rotation
- Smooth interpolation with sine waves
- Multiple animated properties
"""

import math

from pybevy.prelude import *


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up the 3D scene with an animated cube."""
    # Spawn cube
    cube_mesh = Cuboid().mesh()
    cube_material = StandardMaterial(base_color=Color.srgb(0.8, 0.4, 0.2))

    commands.spawn(
        Mesh3d(meshes.add(cube_mesh)),
        MeshMaterial3d(materials.add(cube_material)),
        Transform.from_xyz(0.0, 0.5, 0.0),
    )

    # Add light
    commands.spawn(
        PointLight(shadows_enabled=True),
        Transform.from_xyz(4.0, 8.0, 4.0),
    )

    # Add camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3.ZERO, Vec3.Y),
    )


def animate_cube(cubes: Query[Mut[Transform], With[Mesh3d]], time: Res[Time]) -> None:
    """Animate the cube with rotation and bobbing motion."""
    t = time.elapsed_secs()

    for transform in cubes:
        # Rotate around Y axis
        transform.rotate_y(2.0 * time.delta_secs())

        # Bob up and down
        transform.translation.y = 0.5 + math.sin(t * 2.0) * 0.5


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, animate_cube)
    )


if __name__ == "__main__":
    main().run()
