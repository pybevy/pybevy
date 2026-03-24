"""3D bloom post-processing with emissive materials.

Demonstrates:
- Bloom effect in 3D scenes
- Emissive materials (self-lit objects)
- Bouncing sphere animation
- Tonemapping for HDR rendering
- Multiple emissive intensities

This example shows how bloom makes bright emissive objects glow,
creating a dramatic lighting effect in a dark scene.
"""

import math

from pybevy.camera import Bloom, Tonemapping
from pybevy.color import LinearRgba
from pybevy.prelude import *
from pybevy.pbr import StandardMaterial


@component
class Bouncing(Component):
    """Marker for bouncing spheres."""


def setup_scene(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up 3D scene with emissive spheres."""
    # Camera with bloom
    commands.spawn(
        Camera3d(),
        Tonemapping.TONY_MC_MAPFACE,  # Tonemapper that desaturates to white
        Transform.from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3.ZERO, Vec3.Y),
        Bloom.NATURAL,  # Natural bloom preset
    )

    # Create emissive materials (bright, self-lit)
    mat_blue = materials.add(StandardMaterial(
        emissive=LinearRgba.rgb(0.0, 0.0, 150.0),  # Bright blue glow
    ))

    mat_white = materials.add(StandardMaterial(
        emissive=LinearRgba.rgb(1000.0, 1000.0, 1000.0),  # Very bright white
    ))

    mat_red = materials.add(StandardMaterial(
        emissive=LinearRgba.rgb(50.0, 0.0, 0.0),  # Red glow
    ))

    mat_non_emissive = materials.add(StandardMaterial(
        base_color=Color.BLACK,  # Non-emissive dark spheres
    ))

    # Create sphere mesh
    sphere = meshes.add(Sphere(0.4))

    # Create grid of spheres with different materials
    mat_list = [mat_blue, mat_white, mat_red, mat_non_emissive, mat_non_emissive, mat_non_emissive]
    scales = [0.5, 0.1, 1.0, 1.5, 1.5, 1.5]

    for x in range(-5, 5):
        for z in range(-5, 5):
            # Pseudo-random selection (deterministic)
            idx = (x * 7 + z * 13) % 6

            commands.spawn(
                Mesh3d(sphere),
                MeshMaterial3d(mat_list[idx]),
                Transform.from_xyz(x * 2.0, 0.0, z * 2.0).with_scale(Vec3.splat(scales[idx])),
                Bouncing(),
            )

    print("\n=== 3D Bloom Example ===")
    print("Emissive spheres glow with bloom effect")
    print("Blue, white, and red spheres emit light")
    print("Black spheres do not emit light")
    print("======================\n")


def bounce_spheres(
    time: Res[Time],
    spheres: Query[Mut[Transform], With[Bouncing]],
) -> None:
    """Animate spheres bouncing up and down."""
    t = time.elapsed_secs()

    for transform in spheres:
        # Use position to offset the bounce phase
        phase_offset = (transform.translation.x + transform.translation.z) * 0.5

        # Bounce height
        height = abs(math.sin(t * 2.0 + phase_offset)) * 2.0
        transform.translation.y = height


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup_scene)
        .add_systems(Update, bounce_spheres)
    )


if __name__ == "__main__":
    main().run()
