"""A shader that samples a texture with screen-space UV coordinates.

PyBevy equivalent of Bevy's ``shader_material_screenspace_texture`` example.
The texture is mapped using the fragment's screen position rather than the
mesh's UV coordinates, so the texture stays fixed to the viewport as the
camera orbits.

Bevy (Rust):
    #[derive(Asset, AsBindGroup)]
    struct CustomMaterial {
        #[texture(0)]
        #[sampler(1)]
        texture: Handle<Image>,
    }
    impl Material for CustomMaterial { ... }

PyBevy:
    @material(fragment_shader="shaders/examples/screenspace_texture.wgsl")
    class ScreenspaceMaterial(Material):
        texture: Image  # automatically gets bindings 101 (texture) / 102 (sampler)
"""

from pybevy.pbr import ShaderMaterialPlugin
from pybevy.prelude import *

SHADER_ASSET_PATH = "shaders/examples/screenspace_texture.wgsl"


@material(fragment_shader=SHADER_ASSET_PATH)
class ScreenspaceMaterial(Material):
    """Material that maps a texture using screen-space coordinates."""
    texture: Image


@component
class MainCamera(Component):
    pass


def setup(
    commands: Commands,
    asset_server: Res[AssetServer],
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[ScreenspaceMaterial]],
    standard_materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Ground plane with standard material
    commands.spawn(
        Mesh3d(meshes.add(Plane3d().mesh().size(5.0, 5.0))),
        MeshMaterial3d[StandardMaterial](standard_materials.add(  # type: ignore[misc]
            StandardMaterial(base_color=Color.srgb(0.3, 0.5, 0.3))
        )),
    )

    # Point light
    commands.spawn(
        PointLight(),
        Transform.from_xyz(4, 8, 4),
    )

    # Cube with screenspace-textured material
    commands.spawn(
        Mesh3d(meshes.add(Cuboid())),
        MeshMaterial3d[ScreenspaceMaterial](materials.add(ScreenspaceMaterial(  # type: ignore[misc,call-arg]
            texture=asset_server.load_image("bevy/branding/icon.png"),
        ))),
        Transform.from_xyz(0, 0.5, 0),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(4, 2.5, 4).looking_at(Vec3.ZERO, Vec3.Y),
        MainCamera(),
    )


def rotate_camera(
    query: Query[Mut[Transform], With[MainCamera]],
    time: Res[Time],
) -> None:
    for transform in query:
        transform.rotate_around(
            Vec3.ZERO,
            Quat.from_axis_angle(Vec3.Y, 0.7854 * time.delta_secs()),
        )
        transform.look_at(Vec3.ZERO, Vec3.Y)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(ShaderMaterialPlugin())
        .add_systems(Startup, setup)
        .add_systems(Update, rotate_camera)
    )


if __name__ == "__main__":
    main().run()
