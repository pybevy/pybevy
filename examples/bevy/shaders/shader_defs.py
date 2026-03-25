"""A shader that uses "shader defs", which selectively toggle parts of a shader.

PyBevy equivalent of Bevy's ``shader_defs`` example.
Bool fields in @material become shader defs automatically — no need for
manual specialize() or CustomMaterialKey.

Bevy (Rust):
    #[derive(Asset, AsBindGroup)]
    #[bind_group_data(CustomMaterialKey)]
    struct CustomMaterial {
        #[uniform(0)]
        color: LinearRgba,
        is_red: bool,
    }
    impl Material for CustomMaterial {
        fn specialize(..., key) {
            if key.bind_group_data.is_red {
                fragment.shader_defs.push("IS_RED".into());
            }
        }
    }

PyBevy:
    @material(fragment_shader="shaders/examples/shader_defs.wgsl")
    class CustomMaterial:
        color: LinearRgba = LinearRgba(0.0, 0.0, 1.0, 1.0)
        is_red: bool = False  # automatically becomes #ifdef IS_RED
"""

from pybevy.pbr import ShaderMaterialPlugin
from pybevy.prelude import *

SHADER_ASSET_PATH = "shaders/examples/shader_defs.wgsl"


@material(fragment_shader=SHADER_ASSET_PATH)
class CustomMaterial:
    """Custom material with a color uniform and an IS_RED shader def."""
    color: LinearRgba = LinearRgba(0.0, 0.0, 1.0, 1.0)
    is_red: bool = False


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[CustomMaterial]],  # type: ignore[type-var]
) -> None:
    # Blue cube — uses the uniform color directly
    commands.spawn(
        Mesh3d(meshes.add(Cuboid())),
        MeshMaterial3d[CustomMaterial](materials.add(CustomMaterial(  # type: ignore[misc,call-arg]

            color=LinearRgba(0.0, 0.0, 1.0, 1.0),
            is_red=False,
        ))),
        Transform.from_xyz(-1, 0.5, 0),
    )

    # Red cube — the IS_RED shader def overrides the color to red
    commands.spawn(
        Mesh3d(meshes.add(Cuboid())),
        MeshMaterial3d[CustomMaterial](materials.add(CustomMaterial(  # type: ignore[misc,call-arg]

            color=LinearRgba(0.0, 1.0, 0.0, 1.0),  # green, but overridden by IS_RED
            is_red=True,
        ))),
        Transform.from_xyz(1, 0.5, 0),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-2, 2.5, 5).looking_at(Vec3.ZERO, Vec3.Y),
    )


@entrypoint
def main(app: App) -> App:
    app.add_plugins(DefaultPlugins)
    app.add_plugins(ShaderMaterialPlugin())
    app.add_systems(Startup, setup)
    return app


if __name__ == "__main__":
    main().run()
