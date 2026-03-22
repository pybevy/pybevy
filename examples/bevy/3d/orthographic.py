"""Shows how to create a 3D orthographic view (for isometric-look games or CAD applications).

This example demonstrates:
- Orthographic projection with custom scaling
- Fixed vertical viewport height
- Isometric camera positioning
"""

from __future__ import annotations

from pybevy.decorators import entrypoint
from pybevy.prelude import *


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up a simple 3D scene with orthographic camera."""

    # Camera with orthographic projection
    # 6 world units per pixel of window height
    # Use default_3d() as base and override scaling_mode
    ortho = OrthographicProjection.default_3d()
    ortho.scaling_mode = ScalingMode.FixedVertical(6.0)

    commands.spawn(
        Camera3d(),
        Projection.Orthographic(ortho),
        Transform.from_xyz(5.0, 5.0, 5.0).looking_at(Vec3.ZERO, Vec3.Y),
    )

    # Plane
    commands.spawn(
        Mesh3d(meshes.add(Plane3d().mesh().size(5.0, 5.0))),
        MeshMaterial3d(materials.add(Color.srgb(0.3, 0.5, 0.3))),
    )

    # Cubes
    cube_material = materials.add(Color.srgb(0.8, 0.7, 0.6))
    cube_mesh = meshes.add(Cuboid.from_length(1.0))

    commands.spawn(
        Mesh3d(cube_mesh),
        MeshMaterial3d(cube_material),
        Transform.from_xyz(1.5, 0.5, 1.5),
    )

    commands.spawn(
        Mesh3d(cube_mesh),
        MeshMaterial3d(cube_material),
        Transform.from_xyz(1.5, 0.5, -1.5),
    )

    commands.spawn(
        Mesh3d(cube_mesh),
        MeshMaterial3d(cube_material),
        Transform.from_xyz(-1.5, 0.5, 1.5),
    )

    commands.spawn(
        Mesh3d(cube_mesh),
        MeshMaterial3d(cube_material),
        Transform.from_xyz(-1.5, 0.5, -1.5),
    )

    # Light
    commands.spawn(
        PointLight(),
        Transform.from_xyz(3.0, 8.0, 5.0),
    )


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)


if __name__ == "__main__":
    main().run()
