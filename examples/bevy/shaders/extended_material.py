"""Demonstrates extending StandardMaterial with a custom fragment shader.

PyBevy equivalent of Bevy's ``extended_material`` example.
Adds a posterization/quantize effect on top of standard PBR lighting.

Bevy (Rust):
    #[derive(Asset, AsBindGroup)]
    struct MyExtension {
        #[uniform(100)]
        quantize_steps: u32,
    }
    impl MaterialExtension for MyExtension { ... }

PyBevy:
    @material(fragment_shader="shaders/examples/extended_material.wgsl")
    class QuantizeMaterial:
        quantize_steps: float = 3.0
"""

from pybevy.pbr import ShaderMaterialPlugin
from pybevy.prelude import *

SHADER_ASSET_PATH = "shaders/examples/extended_material.wgsl"


@material(fragment_shader=SHADER_ASSET_PATH)
class QuantizeMaterial:
    """Extended PBR material with a posterization effect."""
    quantize_steps: float = 1.0


@component
class Rotate(Component):
    pass


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[QuantizeMaterial]],  # type: ignore[type-var]
) -> None:
    # Sphere with quantized PBR
    commands.spawn(
        Mesh3d(meshes.add(Sphere(1.0))),
        MeshMaterial3d[QuantizeMaterial](materials.add(QuantizeMaterial(  # type: ignore[misc,call-arg]
            base=StandardMaterial(base_color=Color.srgb(0.8, 0.1, 0.1)),
            quantize_steps=1.0,
        ))),
        Transform.from_xyz(0, 0.5, 0),
    )

    # Rotating directional light
    commands.spawn(
        DirectionalLight(),
        Transform.from_xyz(1, 1, 1).looking_at(Vec3.ZERO, Vec3.Y),
        Rotate(),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-2, 2.5, 5).looking_at(Vec3.ZERO, Vec3.Y),
    )


def rotate_things(
    query: Query[Mut[Transform], With[Rotate]],
    time: Res[Time],
) -> None:
    for transform in query:
        transform.rotate_y(time.delta_secs())


@entrypoint
def main(app: App) -> App:
    app.add_plugins(DefaultPlugins)
    app.add_plugins(ShaderMaterialPlugin())
    app.add_systems(Startup, setup)
    app.add_systems(Update, rotate_things)
    return app


if __name__ == "__main__":
    main().run()
