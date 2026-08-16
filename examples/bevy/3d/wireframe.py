"""Wireframe rendering example.

Demonstrates:
- Per-object wireframe rendering
- Global wireframe toggle
- Custom wireframe colors
- Excluding objects from wireframe
"""

from pybevy.pbr import (
    NoWireframe,
    Wireframe,
    WireframeColor,
    WireframeConfig,
    WireframePlugin,
)
from pybevy.prelude import *


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up scene with wireframe objects."""
    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, 3.0, 10.0).looking_at(Vec3(0.0, 1.0, 0.0), Vec3.Y),
    )

    # Light
    commands.spawn(
        PointLight(intensity=10000.0),
        Transform.from_xyz(4.0, 8.0, 4.0),
    )

    # Material
    material = materials.add(StandardMaterial(
        base_color=Color.srgb(0.5, 0.5, 0.5),
    ))

    # 1. Sphere with default wireframe
    commands.spawn(
        Mesh3d(meshes.add(Sphere(1.0))),
        MeshMaterial3d(material),
        Wireframe(),  # Enable wireframe
        Transform.from_xyz(-3.0, 1.0, 0.0),
    )

    # 2. Cube with green wireframe
    commands.spawn(
        Mesh3d(meshes.add(Cuboid(2.0, 2.0, 2.0))),
        MeshMaterial3d(material),
        Wireframe(),
        WireframeColor(Color.srgb(0.0, 1.0, 0.0)),  # Green wireframe
        Transform.from_xyz(0.0, 1.0, 0.0),
    )

    # 3. Cylinder with red wireframe
    commands.spawn(
        Mesh3d(meshes.add(Cylinder(1.0, 2.0))),
        MeshMaterial3d(material),
        Wireframe(),
        WireframeColor(Color.srgb(1.0, 0.0, 0.0)),  # Red wireframe
        Transform.from_xyz(3.0, 1.0, 0.0),
    )

    # 4. Sphere (will show wireframe when global enabled, but excluded)
    commands.spawn(
        Mesh3d(meshes.add(Sphere(0.5))),
        MeshMaterial3d(material),
        NoWireframe(),  # Explicitly no wireframe
        Transform.from_xyz(-3.0, 1.0, -3.0),
    )

    # 5. Plane (will show wireframe when global enabled)
    commands.spawn(
        Mesh3d(meshes.add(Plane3d().mesh().size(2.0, 2.0).build())),
        MeshMaterial3d(material),
        Transform.from_xyz(0.0, 1.0, -3.0),
    )

    # 6. Cylinder with cyan wireframe (Capsule3d not available in PyBevy)
    commands.spawn(
        Mesh3d(meshes.add(Cylinder(0.5, 2.0))),
        MeshMaterial3d(material),
        Wireframe(),
        WireframeColor(Color.srgb(0.0, 1.0, 1.0)),  # Cyan
        Transform.from_xyz(3.0, 1.5, -3.0),
    )

    # Ground
    ground = materials.add(StandardMaterial(
        base_color=Color.srgb(0.3, 0.3, 0.3),
    ))

    commands.spawn(
        Mesh3d(meshes.add(Plane3d(Vec3.Y, Vec2(20.0, 20.0)))),
        MeshMaterial3d(ground),
        NoWireframe(),  # Never show ground as wireframe
        Transform.from_xyz(0.0, 0.0, 0.0),
    )

    # Instructions
    commands.spawn(
        Text2d("W: Toggle Global Wireframe\nObjects with Wireframe component always show\nNoWireframe components never show"),
        TextFont.from_font_size(18.0),
        TextColor.WHITE,
        Transform.from_xyz(0.0, 400.0, 0.0),
    )

    # Insert wireframe config
    commands.insert_resource(WireframeConfig(
        global_=False,  # Start with global wireframe off
        default_color=Color.WHITE,  # Default wireframe color
    ))


def toggle_wireframe(
    keyboard: Res[ButtonInput],
    config: ResMut[WireframeConfig],
) -> None:
    """Toggle global wireframe with W key."""
    if keyboard.just_pressed(KeyCode.KeyW):
        config.global_ = not config.global_


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(WireframePlugin())
        .add_systems(Startup, setup)
        .add_systems(Update, toggle_wireframe)
    )


if __name__ == "__main__":
    main().run()
