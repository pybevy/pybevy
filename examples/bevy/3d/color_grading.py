"""Demonstrates color grading post-processing with keyboard controls.

This example shows how to use ColorGrading to adjust the appearance of a scene
and demonstrates system chaining with .chain().
"""

import math

from pybevy.prelude import *


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
):
    # Create the scene - a simple setup with some objects
    # Ground plane
    commands.spawn(
        Mesh3d(meshes.add(Plane3d().mesh().size(10.0, 10.0).build())),
        MeshMaterial3d(
            materials.add(StandardMaterial(base_color=Color.srgb(0.3, 0.5, 0.3)))
        ),
    )

    # Create some colored cubes
    colors = [
        Color.srgb(1.0, 0.0, 0.0),  # Red
        Color.srgb(0.0, 1.0, 0.0),  # Green
        Color.srgb(0.0, 0.0, 1.0),  # Blue
        Color.srgb(1.0, 1.0, 0.0),  # Yellow
    ]

    for i, color in enumerate(colors):
        x = (i - 1.5) * 1.5
        commands.spawn(
            Mesh3d(meshes.add(Cuboid())),
            MeshMaterial3d(materials.add(StandardMaterial(base_color=color))),
            Transform.from_xyz(x, 0.5, 0.0),
        )

    # Add a directional light
    commands.spawn(
        DirectionalLight(
            illuminance=1000.0,
            shadows_enabled=True,
        ),
        Transform.from_xyz(0.0, 5.0, 0.0).with_rotation(
            Quat.from_rotation_x(-math.pi / 4.0)
        ),
    )

    # Spawn camera with ColorGrading
    # Start with default color grading
    color_grading = ColorGrading()

    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, 2.0, 8.0).looking_at(Vec3(0.0, 0.5, 0.0), Vec3.Y),
        Hdr(),  # Enable HDR for better color grading results
        color_grading,
    )


def handle_input(
    input: Res[ButtonInput],
    query: Query[Mut[ColorGrading], With[Camera3d]],
    time: Res[Time],
):
    """Handle keyboard input to adjust color grading settings.

    Controls:
    - 1/2: Decrease/Increase global exposure
    - 3/4: Decrease/Increase highlights saturation
    - 5/6: Decrease/Increase midtones saturation
    - 7/8: Decrease/Increase shadows saturation
    - 9/0: Decrease/Increase highlights contrast
    - Q/W: Decrease/Increase midtones contrast
    - E/R: Decrease/Increase shadows contrast
    """
    for grading in query:
        dt = time.delta_secs()
        adjustment = dt * 0.3  # Speed of adjustment

        # Global exposure (1/2)
        if input.pressed(KeyCode.Digit1):
            new_exposure = grading.global_.exposure - adjustment
            grading.global_.exposure = max(-2.0, new_exposure)
        if input.pressed(KeyCode.Digit2):
            new_exposure = grading.global_.exposure + adjustment
            grading.global_.exposure = min(2.0, new_exposure)

        # Highlights saturation (3/4)
        if input.pressed(KeyCode.Digit3):
            new_sat = grading.highlights.saturation - adjustment
            grading.highlights.saturation = max(0.0, new_sat)
        if input.pressed(KeyCode.Digit4):
            new_sat = grading.highlights.saturation + adjustment
            grading.highlights.saturation = min(2.0, new_sat)

        # Midtones saturation (5/6)
        if input.pressed(KeyCode.Digit5):
            new_sat = grading.midtones.saturation - adjustment
            grading.midtones.saturation = max(0.0, new_sat)
        if input.pressed(KeyCode.Digit6):
            new_sat = grading.midtones.saturation + adjustment
            grading.midtones.saturation = min(2.0, new_sat)

        # Shadows saturation (7/8)
        if input.pressed(KeyCode.Digit7):
            new_sat = grading.shadows.saturation - adjustment
            grading.shadows.saturation = max(0.0, new_sat)
        if input.pressed(KeyCode.Digit8):
            new_sat = grading.shadows.saturation + adjustment
            grading.shadows.saturation = min(2.0, new_sat)

        # Highlights contrast (9/0)
        if input.pressed(KeyCode.Digit9):
            new_contrast = grading.highlights.contrast - adjustment
            grading.highlights.contrast = max(0.0, new_contrast)
        if input.pressed(KeyCode.Digit0):
            new_contrast = grading.highlights.contrast + adjustment
            grading.highlights.contrast = min(2.0, new_contrast)

        # Midtones contrast (Q/W)
        if input.pressed(KeyCode.KeyQ):
            new_contrast = grading.midtones.contrast - adjustment
            grading.midtones.contrast = max(0.0, new_contrast)
        if input.pressed(KeyCode.KeyW):
            new_contrast = grading.midtones.contrast + adjustment
            grading.midtones.contrast = min(2.0, new_contrast)

        # Shadows contrast (E/R)
        if input.pressed(KeyCode.KeyE):
            new_contrast = grading.shadows.contrast - adjustment
            grading.shadows.contrast = max(0.0, new_contrast)
        if input.pressed(KeyCode.KeyR):
            new_contrast = grading.shadows.contrast + adjustment
            grading.shadows.contrast = min(2.0, new_contrast)


def print_settings(query: Query[ColorGrading, With[Camera3d]]):
    """Print current color grading settings (runs after handle_input via chain)."""
    # This system demonstrates chaining - it runs after handle_input,
    # so it always shows the updated values
    for _grading in query:
        # In a real app, this would update UI text
        # For now, we just validate that the chained system runs in order
        pass


@entrypoint
def main(app: App) -> App:
    """
    This example demonstrates:
    1. ColorGrading component for post-processing
    2. System chaining with chain() to ensure execution order

    The handle_input and print_settings systems are chained together,
    ensuring print_settings always sees the updated values from handle_input.
    """
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            # Use chain() to ensure handle_input runs before print_settings
            chain(handle_input, print_settings),
        )
    )


if __name__ == "__main__":
    main().run()
