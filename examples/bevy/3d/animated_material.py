"""Shows how to animate material properties.

Demonstrates animating StandardMaterial base_color using Assets.get_mut()
to modify materials in-place. Creates a 3x3 grid of cubes with different
colors that continuously shift through the color spectrum.
"""


from pybevy.app import App, DefaultPlugins
from pybevy.assets import Assets, AssetServer
from pybevy.camera import Camera3d
from pybevy.color import Color
from pybevy.decorators import entrypoint
from pybevy.ecs import Commands, Query, Res, ResMut
from pybevy.light import EnvironmentMapLight
from pybevy.math import Cuboid, Vec3
from pybevy.mesh import Mesh, Mesh3d, MeshMaterial3d
from pybevy.prelude import Startup, Update
from pybevy.render import StandardMaterial
from pybevy.time import Time
from pybevy.transform import Transform


def setup(
    commands: Commands,
    asset_server: Res[AssetServer],
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up the scene with camera, environment lighting, and colored cubes."""
    # Camera with environment lighting
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(3.0, 1.0, 3.0).looking_at(Vec3(0.0, -0.5, 0.0), Vec3.Y),
        EnvironmentMapLight(
            diffuse_map=asset_server.load_image("bevy/environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
            specular_map=asset_server.load_image("bevy/environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            intensity=2000.0,
        ),
    )

    # Create cube mesh
    cube = meshes.add(Cuboid(0.5, 0.5, 0.5).mesh())

    # Golden angle for pleasing color distribution
    GOLDEN_ANGLE = 137.50777

    # Spawn 3x3 grid of cubes with different hue values
    hue = 0.0
    for x in range(-1, 2):
        for z in range(-1, 2):
            commands.spawn(
                Mesh3d(cube),
                MeshMaterial3d(materials.add(
                    StandardMaterial(base_color=Color.hsl(hue, 1.0, 0.5))
                )),
                Transform.from_translation(Vec3(float(x), 0.0, float(z))),
            )
            hue = (hue + GOLDEN_ANGLE) % 360.0


def animate_materials(
    material_handles: Query[MeshMaterial3d],
    time: Res[Time],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Animate material colors by rotating hue over time."""
    for material_handle in material_handles:
        # Get mutable access to the material
        material = materials.get_mut(material_handle.handle())
        if material:
            # Simple hue rotation using time - all cubes cycle together
            t = time.elapsed_secs()
            new_hue = (t * 50.0) % 360.0  # Rotate through full spectrum

            # Set new color
            material.base_color = Color.hsl(new_hue, 1.0, 0.5)


@entrypoint
def main(app: App) -> App:
    """Configure and return the app."""
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, animate_materials)
    )


if __name__ == "__main__":
    main().run()
