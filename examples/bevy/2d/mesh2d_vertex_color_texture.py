"""Shows how to render a polygonal Mesh with vertex colors and texture.

Demonstrates:
- Creating a mesh from a Rectangle primitive
- Adding vertex colors to mesh vertices
- Per-vertex tinting when combined with textures
- Mesh attribute manipulation
"""

import numpy as np

from pybevy.prelude import *


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[ColorMaterial]],
    asset_server: Res[AssetServer],
) -> None:
    """Set up the scene with vertex-colored meshes."""
    texture_handle = asset_server.load_image("bevy/branding/banner.png")

    mesh = Rectangle().mesh().build()

    red = LinearRgba.rgb(1.0, 0.0, 0.0)
    green = LinearRgba.rgb(0.0, 1.0, 0.0)
    blue = LinearRgba.rgb(0.0, 0.0, 1.0)
    white = LinearRgba.WHITE
    vertex_colors = np.array([
        [red.red, red.green, red.blue, red.alpha],
        [green.red, green.green, green.blue, green.alpha],
        [blue.red, blue.green, blue.blue, blue.alpha],
        [white.red, white.green, white.blue, white.alpha],
    ], dtype=np.float32)

    mesh.insert_attribute(Mesh.ATTRIBUTE_COLOR, vertex_colors)

    mesh_handle = meshes.add(mesh)

    commands.spawn(Camera2d())

    commands.spawn(
        Mesh2d(mesh_handle),
        MeshMaterial2d(materials.add(ColorMaterial())),
        Transform.from_translation(Vec3(-96.0, 0.0, 0.0)).with_scale(Vec3.splat(128.0)),
    )

    commands.spawn(
        Mesh2d(mesh_handle),
        MeshMaterial2d(materials.add(ColorMaterial(texture=texture_handle))),
        Transform.from_translation(Vec3(96.0, 0.0, 0.0)).with_scale(Vec3.splat(128.0)),
    )


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)


if __name__ == "__main__":
    main().run()
