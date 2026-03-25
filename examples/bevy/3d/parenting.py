"""Illustrates how to create parent-child relationships between entities.

This example demonstrates:
- Creating parent-child entity hierarchies
- How parent transforms propagate to children
- Using custom components as markers
- Rotating parent entities affects children
"""

from __future__ import annotations

from pybevy.prelude import *


@component
class Rotator(Component):
    """Component that marks entities that should rotate."""


def rotator_system(
    time: Res[Time],
    query: Query[Mut[Transform], With[Rotator]]
) -> None:
    """Rotates the parent, which results in the child also rotating."""
    for transform in query:
        transform.rotate_x(3.0 * time.delta_secs())

def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up a simple scene with a parent cube and a child cube."""

    cube_handle = meshes.add(Cuboid(2.0, 2.0, 2.0))
    cube_material_handle = materials.add(StandardMaterial(
        base_color=Color.srgb(0.8, 0.7, 0.6)
    ))

    # Parent cube
    parent = commands.spawn(
        Mesh3d(cube_handle),
        MeshMaterial3d(cube_material_handle),
        Transform.from_xyz(0.0, 0.0, 1.0),
        Rotator(),
    )

    # Add child cube to parent
    parent.with_children(
        lambda child_builder: [
            child_builder.spawn(
                Mesh3d(cube_handle),
                MeshMaterial3d(cube_material_handle),
                Transform.from_xyz(0.0, 0.0, 3.0),
            )
        ]
    )

    # Light
    commands.spawn(
        PointLight(),
        Transform.from_xyz(4.0, 5.0, -4.0),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(5.0, 10.0, 10.0).looking_at(Vec3.ZERO, Vec3.Y),
    )


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, rotator_system)
    )


if __name__ == "__main__":
    main().run()
