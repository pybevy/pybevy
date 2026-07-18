"""Illustrates the use of vertex colors.

Demonstrates how to assign custom colors to individual vertices of a mesh.
The cube's vertices are colored based on their positions, creating a
multi-colored gradient effect.
"""

import numpy as np

from pybevy.prelude import *


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up a simple 3D scene with vertex-colored cube."""
    # Ground plane
    commands.spawn(
        Mesh3d(meshes.add(Plane3d().mesh().size(5.0, 5.0).build())),
        MeshMaterial3d(materials.add(
            StandardMaterial(base_color=Color.srgb(0.3, 0.5, 0.3))
        )),
    )

    # Create cube with vertex colors
    # Assign vertex colors based on vertex positions
    colorful_cube = Cuboid().mesh().build()

    # Get vertex positions (zero-copy bounded array, converted to numpy for the math)
    positions = colorful_cube.attribute(Mesh.ATTRIBUTE_POSITION).to_numpy()
    # Convert positions to colors: map [-0.5, 0.5] to [0, 1]
    colors = np.zeros((len(positions), 4), dtype=np.float32)
    colors[:, 0] = (1.0 - positions[:, 0]) / 2.0  # R from x
    colors[:, 1] = (1.0 - positions[:, 1]) / 2.0  # G from y
    colors[:, 2] = (1.0 - positions[:, 2]) / 2.0  # B from z
    colors[:, 3] = 1.0  # Alpha

    # Insert vertex colors
    colorful_cube.insert_attribute(Mesh.ATTRIBUTE_COLOR, colors)

    commands.spawn(
        Mesh3d(meshes.add(colorful_cube)),
        # White base color so vertex colors show through
        # (vertex colors are multiplied by base_color)
        MeshMaterial3d(materials.add(
            StandardMaterial(base_color=Color.WHITE)
        )),
        Transform.from_xyz(0.0, 0.5, 0.0),
    )

    # Light with shadows
    commands.spawn(
        PointLight(shadow_maps_enabled=True),
        Transform.from_xyz(4.0, 5.0, 4.0).looking_at(Vec3.ZERO, Vec3.Y),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3.ZERO, Vec3.Y),
    )


@entrypoint
def main(app: App) -> App:
    """Configure and return the app."""
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)


if __name__ == "__main__":
    main().run()
