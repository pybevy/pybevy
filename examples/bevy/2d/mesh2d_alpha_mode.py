"""Test how transforms interact with alpha modes for Mesh2d entities.

Demonstrates:
- AlphaMode2d.Opaque - solid rendering (no transparency)
- AlphaMode2d.Blend - alpha blending for transparency
- AlphaMode2d.Mask - threshold-based transparency
- Depth buffer usage for 2D meshes
- ColorMaterial with alpha modes and textures
"""

from pybevy.prelude import *
from pybevy.sprite import AlphaMode2d


def setup(
    commands: Commands,
    asset_server: Res[AssetServer],
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[ColorMaterial]],
) -> None:
    """Set up the scene with various alpha mode examples."""
    commands.spawn(Camera2d())

    texture_handle = asset_server.load_image("icon.png")
    mesh_handle = meshes.add(Rectangle.from_size(Vec2.splat(256.0)).mesh().build())

    commands.spawn(
        Mesh2d(mesh_handle),
        MeshMaterial2d(materials.add(ColorMaterial(
            color=Color.WHITE,
            alpha_mode=AlphaMode2d.Opaque(),
            texture=texture_handle,
        ))),
        Transform.from_xyz(-400.0, 0.0, 0.0),
    )

    commands.spawn(
        Mesh2d(mesh_handle),
        MeshMaterial2d(materials.add(ColorMaterial(
            color=Color.linear_rgb(0.0, 0.0, 1.0),
            alpha_mode=AlphaMode2d.Opaque(),
            texture=texture_handle,
        ))),
        Transform.from_xyz(-300.0, 0.0, 1.0),
    )

    commands.spawn(
        Mesh2d(mesh_handle),
        MeshMaterial2d(materials.add(ColorMaterial(
            color=Color.linear_rgb(0.0, 1.0, 0.0),
            alpha_mode=AlphaMode2d.Opaque(),
            texture=texture_handle,
        ))),
        Transform.from_xyz(-200.0, 0.0, -1.0),
    )

    commands.spawn(
        Mesh2d(mesh_handle),
        MeshMaterial2d(materials.add(ColorMaterial(
            color=Color.WHITE,
            alpha_mode=AlphaMode2d.Mask(0.5),
            texture=texture_handle,
        ))),
        Transform.from_xyz(200.0, 0.0, 0.0),
    )

    commands.spawn(
        Mesh2d(mesh_handle),
        MeshMaterial2d(materials.add(ColorMaterial(
            color=Color.linear_rgba(0.0, 0.0, 1.0, 0.7),
            alpha_mode=AlphaMode2d.Blend(),
            texture=texture_handle,
        ))),
        Transform.from_xyz(300.0, 0.0, 1.0),
    )

    commands.spawn(
        Mesh2d(mesh_handle),
        MeshMaterial2d(materials.add(ColorMaterial(
            color=Color.linear_rgba(0.0, 1.0, 0.0, 0.7),
            alpha_mode=AlphaMode2d.Blend(),
            texture=texture_handle,
        ))),
        Transform.from_xyz(400.0, 0.0, -1.0),
    )


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)


if __name__ == "__main__":
    main().run()
