"""A shader that uses dynamic data like the time since startup.

PyBevy equivalent of Bevy's ``animate_shader`` example.
The time data is in the globals binding which is part of
the mesh_view_bindings shader import — no custom uniforms needed.

Bevy (Rust):
    #[derive(Asset, AsBindGroup)]
    struct CustomMaterial {}
    impl Material for CustomMaterial {
        fn fragment_shader() -> ShaderRef { ... }
    }

PyBevy:
    @material(fragment_shader="shaders/examples/animate_shader.wgsl")
    class AnimatedMaterial:
        pass  # no fields — all animation is in the shader via globals.time
"""

from pybevy.pbr import ShaderMaterialPlugin
from pybevy.prelude import *

SHADER_ASSET_PATH = "shaders/examples/animate_shader.wgsl"


@material(fragment_shader=SHADER_ASSET_PATH)
class AnimatedMaterial:
    """Material with time-based color animation done entirely in WGSL."""


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[AnimatedMaterial]],  # type: ignore[type-var]
) -> None:
    # Cube with animated shader
    commands.spawn(
        Mesh3d(meshes.add(Cuboid())),
        MeshMaterial3d[AnimatedMaterial](materials.add(AnimatedMaterial())),  # type: ignore[misc,call-arg]
        Transform.from_xyz(0, 0.5, 0),
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
