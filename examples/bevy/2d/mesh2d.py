"""Shows how to render a polygonal Mesh, generated from a Rectangle primitive, in a 2D scene.

Demonstrates:
- Creating a 2D mesh from a primitive shape
- ColorMaterial for 2D mesh rendering
- Mesh2d and MeshMaterial2d components
- Transform scaling
"""

from pybevy.prelude import *


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[ColorMaterial]],
) -> None:
    """Set up 2D scene with a rectangular mesh."""
    commands.spawn(Camera2d())

    # Create a purple rectangle mesh
    rect_mesh = Rectangle().mesh()
    purple = Color.srgb(0.5, 0.0, 0.5)  # PURPLE from basic palette

    commands.spawn(
        Mesh2d(meshes.add(rect_mesh)),
        MeshMaterial2d(materials.add(ColorMaterial(color=purple))),
        Transform().with_scale(Vec3.splat(128.0)),
    )


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)


if __name__ == "__main__":
    main().run()
