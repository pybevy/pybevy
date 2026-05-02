"""Animate material colors over time using manual animation logic.

Demonstrates:
- Manual color animation without AnimationClip assets
- Modifying StandardMaterial base_color over time
- Using Time resource for smooth animations
- Assets mutation through ResMut[Assets[StandardMaterial]]
- Cycling through colors with smooth sine wave transitions

This example shows how to create custom animations by directly
modifying material properties in Update systems using Assets.get_mut().
"""

import math

from pybevy.prelude import *


@component
class AnimatedColor(Component):
    """Marker component storing animation speed."""

    def __init__(self, speed: float = 1.0):
        self.speed = speed


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up the scene with spheres that will have animated colors."""
    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3.ZERO, Vec3.Y),
    )

    # Light
    commands.spawn(
        PointLight(intensity=500000.0),
        Transform.from_xyz(0.0, 2.5, 0.0),
    )

    # Create a sphere mesh
    sphere = meshes.add(Sphere(0.5))

    # Create several spheres with different animation speeds
    for x in range(-2, 3):
        # Create material for this sphere
        material = materials.add(StandardMaterial(
            base_color=Color.WHITE,
        ))

        # Spawn sphere with animated color marker
        speed = 1.0 + (x * 0.3)  # Different speeds for variation
        commands.spawn(
            Mesh3d(sphere),
            MeshMaterial3d(material),
            Transform.from_xyz(x * 1.5, 0.0, 0.0),
            AnimatedColor(speed),
        )


def animate_colors(
    time: Res[Time],
    query: Query[tuple[MeshMaterial3d, AnimatedColor]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Animate the colors of all materials with AnimatedColor component.

    This demonstrates manual animation by modifying asset properties
    directly in an Update system.
    """
    for material_component, animated in query:
        # Get mutable access to the material asset
        material = materials.get_mut(material_component.handle)
        if material is None:
            continue

        # Calculate color based on time and animation speed
        t = time.elapsed_secs() * animated.speed

        # Create RGB values using sine waves with phase offsets
        r = (math.sin(t) + 1.0) / 2.0
        g = (math.sin(t + 2.094) + 1.0) / 2.0  # +2π/3 phase offset
        b = (math.sin(t + 4.189) + 1.0) / 2.0  # +4π/3 phase offset

        # Update the material's base color
        material.base_color = Color.linear_rgb(r, g, b)


def rotate_objects(
    time: Res[Time],
    query: Query[Mut[Transform], With[AnimatedColor]],
) -> None:
    """Rotate all animated objects around Y axis."""
    rotation_speed = 0.5
    angle = time.elapsed_secs() * rotation_speed

    for transform in query:
        transform.rotation = Quat.from_rotation_y(angle)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(GlobalAmbientLight(
            color=Color.WHITE,
            brightness=150.0,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, (animate_colors, rotate_objects))
    )


if __name__ == "__main__":
    main().run()
