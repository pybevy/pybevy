"""A shader and a material that uses it.

PyBevy equivalent of Bevy's ``shader_material`` example.
Defines a custom material with uniform fields and a WGSL fragment shader.

Bevy (Rust):
    #[derive(Asset, AsBindGroup)]
    struct CustomMaterial {
        #[uniform(0)]
        color: LinearRgba,
    }
    impl Material for CustomMaterial { ... }

PyBevy:
    @material(fragment_shader="shaders/examples/shader_material.wgsl")
    class CustomMaterial:
        color: LinearRgba = LinearRgba(0.0, 0.0, 1.0, 1.0)
        intensity: float = 1.0
"""

from pybevy.pbr import ShaderMaterialPlugin
from pybevy.prelude import *

SHADER_ASSET_PATH = "shaders/examples/shader_material.wgsl"


@material(fragment_shader=SHADER_ASSET_PATH)
class CustomMaterial:
    """Custom material with a color tint and emissive intensity."""
    color: LinearRgba = LinearRgba(0.0, 0.0, 1.0, 1.0)
    intensity: float = 1.0


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[CustomMaterial]],  # type: ignore[type-var]
) -> None:
    # Point light
    commands.spawn(
        PointLight(shadow_maps_enabled=True, intensity=2_000_000.0),
        Transform.from_xyz(4, 8, 4),
    )

    # Cube with custom material
    commands.spawn(
        Mesh3d(meshes.add(Cuboid())),
        MeshMaterial3d[CustomMaterial](materials.add(CustomMaterial(  # type: ignore[misc,call-arg]
            color=LinearRgba(0.0, 0.0, 1.0, 1.0),
            intensity=1.5,
        ))),
        Transform.from_xyz(0, 0.5, 0),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-2, 2.5, 5).looking_at(Vec3.ZERO, Vec3.Y),
    )


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(ShaderMaterialPlugin())
        .add_systems(Startup, setup)
    )


if __name__ == "__main__":
    main().run()
