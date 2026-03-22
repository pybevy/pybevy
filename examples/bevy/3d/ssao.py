"""Screen Space Ambient Occlusion (SSAO) example.

Demonstrates:
- SSAO for realistic ambient lighting
- Different quality levels
- Interactive quality switching
- Comparison with/without SSAO
"""

from pybevy.input import ButtonInput, KeyCode
from pybevy.pbr import (
    ScreenSpaceAmbientOcclusion,
    ScreenSpaceAmbientOcclusionQualityLevel,
)
from pybevy.prelude import *
from pybevy.text import Text2d, TextColor, TextFont


@component
class SsaoCamera(Component):
    """Marker for the camera with SSAO."""


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up scene with SSAO."""
    # Camera with SSAO enabled (High quality by default)
    # SSAO requires MSAA to be disabled
    commands.spawn(
        Camera3d(),
        Msaa.Off,
        ScreenSpaceAmbientOcclusion(
            quality_level=ScreenSpaceAmbientOcclusionQualityLevel.High(),
        ),
        Transform.from_xyz(0.0, 5.0, 10.0).looking_at(Vec3.ZERO, Vec3.Y),
        SsaoCamera(),
    )

    # Light
    commands.spawn(
        DirectionalLight(illuminance=5000.0),
        Transform.from_xyz(4.0, 8.0, 4.0).looking_at(Vec3.ZERO, Vec3.Y),
    )

    # Material
    material = materials.add(StandardMaterial(
        base_color=Color.srgb(0.8, 0.8, 0.8),
        perceptual_roughness=0.9,
    ))

    # Create a complex scene to show SSAO effect
    sphere = meshes.add(Sphere(1.0))
    cube = meshes.add(Cuboid(2.0, 2.0, 2.0))

    # Spheres in a grid
    for x in range(-2, 3):
        for z in range(-2, 3):
            commands.spawn(
                Mesh3d(sphere),
                MeshMaterial3d(material),
                Transform.from_xyz(x * 3.0, 0.5, z * 3.0),
            )

    # Cubes above spheres
    for x in range(-1, 2):
        for z in range(-1, 2):
            commands.spawn(
                Mesh3d(cube),
                MeshMaterial3d(material),
                Transform.from_xyz(x * 3.5, 3.0, z * 3.5),
            )

    # Ground plane
    ground = materials.add(StandardMaterial(
        base_color=Color.srgb(0.3, 0.5, 0.3),
        perceptual_roughness=1.0,
    ))

    commands.spawn(
        Mesh3d(meshes.add(Plane3d(Vec3.Y, Vec2(20.0, 20.0)))),
        MeshMaterial3d(ground),
        Transform.from_xyz(0.0, 0.0, 0.0),
    )

    # Instructions
    commands.spawn(
        Text2d("1-4: Quality (Low/Med/High/Ultra)\n0: Toggle SSAO On/Off\nSSAO adds realistic shadows in crevices"),
        TextFont.from_font_size(18.0),
        TextColor.WHITE,
        Transform.from_xyz(0.0, 400.0, 0.0),
    )


def toggle_ssao(
    keyboard: Res[ButtonInput],
    camera_query: Query[tuple[Entity, Mut[ScreenSpaceAmbientOcclusion]], With[SsaoCamera]],
    commands: Commands,
) -> None:
    """Toggle SSAO on/off with 0 key."""
    if not keyboard.just_pressed(KeyCode.Digit0):
        return

    for entity, _ssao in camera_query:
        # Toggle by removing/adding component
        # Note: This is a workaround - ideally we'd have an enabled/disabled field
        commands.entity(entity).remove(ScreenSpaceAmbientOcclusion)


def change_quality(
    keyboard: Res[ButtonInput],
    ssao_query: Query[Mut[ScreenSpaceAmbientOcclusion]],
) -> None:
    """Change SSAO quality with number keys 1-4."""
    quality = None

    if keyboard.just_pressed(KeyCode.Digit1):
        quality = ScreenSpaceAmbientOcclusionQualityLevel.Low()
    elif keyboard.just_pressed(KeyCode.Digit2):
        quality = ScreenSpaceAmbientOcclusionQualityLevel.Medium()
    elif keyboard.just_pressed(KeyCode.Digit3):
        quality = ScreenSpaceAmbientOcclusionQualityLevel.High()
    elif keyboard.just_pressed(KeyCode.Digit4):
        quality = ScreenSpaceAmbientOcclusionQualityLevel.Ultra()

    if quality is not None:
        for ssao in ssao_query:
            ssao.quality_level = quality


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (toggle_ssao, change_quality))
    )


if __name__ == "__main__":
    main().run()
