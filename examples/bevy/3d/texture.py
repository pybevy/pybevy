"""Shows various ways to configure texture materials in 3D.

Demonstrates:
- Loading textures with AssetServer
- StandardMaterial with base_color_texture
- AlphaMode for transparency
- Color modulation of textures
- Rectangle mesh for quads
- Transform rotation and positioning
"""

import math

from pybevy.assets import Assets, AssetServer
from pybevy.ecs import Commands, Res, ResMut
from pybevy.math import Quat, Rectangle, Vec3
from pybevy.mesh import Mesh3d, MeshMaterial3d
from pybevy.prelude import *
from pybevy.render import AlphaMode, StandardMaterial


def setup(
    commands: Commands,
    asset_server: Res[AssetServer],
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up scene with textured entities."""
    # Load texture (using bevy_bird_dark.png as a substitute for bevy_logo_dark_big.png)
    texture_handle = asset_server.load_image("bevy/branding/bevy_bird_dark.png")
    aspect = 0.25

    # Create a quad mesh
    quad_width = 8.0
    quad_handle = meshes.add(Rectangle(quad_width, quad_width * aspect).mesh())

    # Normal texture material
    material_handle = materials.add(
        StandardMaterial(
            base_color_texture=texture_handle,
            alpha_mode=AlphaMode.Blend(),
            unlit=True,
        )
    )

    # Red modulated texture material
    red_material_handle = materials.add(
        StandardMaterial(
            base_color=Color.srgba(1.0, 0.0, 0.0, 0.5),
            base_color_texture=texture_handle,
            alpha_mode=AlphaMode.Blend(),
            unlit=True,
        )
    )

    # Blue modulated texture material
    blue_material_handle = materials.add(
        StandardMaterial(
            base_color=Color.srgba(0.0, 0.0, 1.0, 0.5),
            base_color_texture=texture_handle,
            alpha_mode=AlphaMode.Blend(),
            unlit=True,
        )
    )

    # Textured quad - normal
    commands.spawn(
        Mesh3d(quad_handle),
        MeshMaterial3d(material_handle),
        Transform.from_xyz(0.0, 0.0, 1.5).with_rotation(
            Quat.from_rotation_x(-math.pi / 5.0)
        ),
    )

    # Textured quad - red modulated
    commands.spawn(
        Mesh3d(quad_handle),
        MeshMaterial3d(red_material_handle),
        Transform.from_rotation(Quat.from_rotation_x(-math.pi / 5.0)),
    )

    # Textured quad - blue modulated
    commands.spawn(
        Mesh3d(quad_handle),
        MeshMaterial3d(blue_material_handle),
        Transform.from_xyz(0.0, 0.0, -1.5).with_rotation(
            Quat.from_rotation_x(-math.pi / 5.0)
        ),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(3.0, 5.0, 8.0).looking_at(Vec3.ZERO, Vec3.Y),
    )


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)


if __name__ == "__main__":
    main().run()
