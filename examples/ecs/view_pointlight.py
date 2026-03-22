"""
Example: Pulsating red point light using View API

Simple demonstration of using View API to animate a PointLight's intensity.
"""

import math

from pybevy.prelude import *
from pybevy.time import Time


@component
class PulsatingLight(Component):
    """Marker for our pulsating light."""



def setup(
    commands: Commands, meshes: ResMut[Assets[Mesh]], materials: ResMut[Assets[StandardMaterial]]
) -> None:
    """Create a single red point light above a cube."""
    # Red pulsating light
    commands.spawn(
        PointLight(
            color=Color.srgb(1.0, 0.0, 0.0),  # Red
            intensity=500000.0,
            range=20.0,
            shadows_enabled=True,
        ),
        Transform.from_xyz(0.0, 2.0, 0.0),
        PulsatingLight(),
    )

    # Ground plane
    commands.spawn(
        Mesh3d(meshes.add(Circle(4.0))),
        MeshMaterial3d(materials.add(Color.WHITE)),
        Transform.from_rotation(Quat.from_rotation_x(-math.pi / 2)),
    )

    # Cube
    commands.spawn(
        Mesh3d(meshes.add(Cuboid.from_length(1.0))),
        MeshMaterial3d(materials.add(Color.srgb(0.8, 0.8, 0.8))),
        Transform.from_xyz(0.0, 0.5, 0.0),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, 2.0, 5.0).looking_at(Vec3(0.0, 0.0, 0.0), Vec3.Y),
    )


def pulsate_light(
    view: View[Mut[PointLight], With[PulsatingLight]], time: Res[Time]) -> None:
    """Make the red light pulsate using View API."""
    light = view.column_mut(PointLight)

    # Pulsate intensity based on sine wave
    base_intensity = 500000.0
    pulse_amount = 300000.0

    # Create time-based expression
    elapsed = time.elapsed_secs()
    angle = light.intensity * 0.0 + elapsed * 3.0

    # Apply sine wave to intensity
    light.intensity = base_intensity + angle.sin() * pulse_amount


@entrypoint
def main(app: App) -> App:
    """Run the pulsating light example."""
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, pulsate_light)
    )


if __name__ == "__main__":
    main().run()
