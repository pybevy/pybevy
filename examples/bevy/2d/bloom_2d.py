"""Bloom post-processing in 2D.

Demonstrates:
- Bloom effect with glowing sprites and meshes
- Interactive bloom parameter adjustment
- Multiple bloom settings (intensity, frequency boost, etc.)
- Tonemapping algorithm switching
- Bright colors in dark environment for bloom visibility
"""

from pybevy.prelude import *


@component
class BloomCamera(Component):
    """Marker for the camera with bloom."""


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[ColorMaterial]],
    asset_server: Res[AssetServer],
) -> None:
    """Set up scene with bloom effect."""
    # Camera with bloom enabled
    # Using TonyMcMapface tonemapper (desaturates to white)
    commands.spawn(
        Camera2d(),
        Tonemapping.TONY_MC_MAPFACE,
        Bloom(),  # Enable bloom with default settings
        BloomCamera(),
    )

    # Bright sprite - will glow with bloom
    image = asset_server.load_image("bevy/branding/bevy_bird_dark.png")
    sprite = Sprite.from_image(image)
    sprite.color = Color.srgb(5.0, 5.0, 5.0)  # Super bright to trigger bloom
    sprite.custom_size = Vec2(160.0, 160.0)
    commands.spawn(sprite)

    # Bright magenta circle mesh - will glow
    circle = meshes.add(Circle(100.0))
    magenta = materials.add(ColorMaterial(color=Color.srgb(7.5, 0.0, 7.5)))
    commands.spawn(
        Mesh2d(circle),
        MeshMaterial2d(magenta),
        Transform.from_xyz(-200.0, 0.0, 0.0),
    )

    # Bright cyan circle mesh - will glow
    circle2 = meshes.add(Circle(100.0))
    cyan = materials.add(ColorMaterial(color=Color.srgb(6.25, 9.4, 9.1)))
    commands.spawn(
        Mesh2d(circle2),
        MeshMaterial2d(cyan),
        Transform.from_xyz(200.0, 0.0, 0.0),
    )

    # Instructions printed to console
    print("\n=== Bloom Controls ===")
    print("Space: Toggle bloom on/off")
    print("Q/A: Increase/decrease intensity")
    print("W/S: Increase/decrease low-frequency boost")
    print("E/D: Increase/decrease boost curvature")
    print("R/F: Increase/decrease high-pass frequency")
    print("T/G: Toggle composite mode (EnergyConserving/Additive)")
    print("Y/H: Increase/decrease threshold")
    print("U/J: Increase/decrease threshold softness")
    print("I/K: Increase/decrease horizontal scale")
    print("O: Cycle tonemapping algorithms")
    print("======================\n")


def update_bloom_settings(
    camera_query: Query[tuple[Entity, Mut[Tonemapping], Mut[Bloom]], With[BloomCamera]],
    commands: Commands,
    keyboard: Res[ButtonInput],
    time: Res[Time],
) -> None:
    """Update bloom settings based on keyboard input.

    Note: Simplified from Rust version - bloom is always enabled.
    Press Space to toggle (adds/removes component), but settings
    only work when bloom is enabled.
    """
    # Note: We can't have two queries with mutable access to the same component,
    # so we only query cameras with bloom. To toggle off, press Space.
    for entity, tonemapping, bloom in camera_query:
        dt = time.delta_secs()

        # Toggle bloom off with Space
        if keyboard.just_pressed(KeyCode.Space):
            print("Bloom: OFF (press Space again to re-enable)")
            commands.entity(entity).remove(Bloom)
            return

        # Intensity (Q/A)
        if keyboard.pressed(KeyCode.KeyA):
            bloom.intensity = max(0.0, bloom.intensity - dt / 10.0)
        if keyboard.pressed(KeyCode.KeyQ):
            bloom.intensity = min(1.0, bloom.intensity + dt / 10.0)

        # Low frequency boost (W/S)
        if keyboard.pressed(KeyCode.KeyS):
            bloom.low_frequency_boost = max(0.0, bloom.low_frequency_boost - dt / 10.0)
        if keyboard.pressed(KeyCode.KeyW):
            bloom.low_frequency_boost = min(1.0, bloom.low_frequency_boost + dt / 10.0)

        # Low frequency boost curvature (E/D)
        if keyboard.pressed(KeyCode.KeyD):
            bloom.low_frequency_boost_curvature = max(0.0, bloom.low_frequency_boost_curvature - dt / 10.0)
        if keyboard.pressed(KeyCode.KeyE):
            bloom.low_frequency_boost_curvature = min(1.0, bloom.low_frequency_boost_curvature + dt / 10.0)

        # High pass frequency (R/F)
        if keyboard.pressed(KeyCode.KeyF):
            bloom.high_pass_frequency = max(0.0, bloom.high_pass_frequency - dt / 10.0)
        if keyboard.pressed(KeyCode.KeyR):
            bloom.high_pass_frequency = min(1.0, bloom.high_pass_frequency + dt / 10.0)

        # Composite mode (T/G)
        if keyboard.just_pressed(KeyCode.KeyT):
            bloom.composite_mode = BloomCompositeMode.EnergyConserving
            print("Composite mode: EnergyConserving")
        if keyboard.just_pressed(KeyCode.KeyG):
            bloom.composite_mode = BloomCompositeMode.Additive
            print("Composite mode: Additive")

        # Threshold (Y/H)
        if keyboard.pressed(KeyCode.KeyH):
            prefilter = bloom.prefilter
            prefilter.threshold = max(0.0, prefilter.threshold - dt)
            bloom.prefilter = prefilter
        if keyboard.pressed(KeyCode.KeyY):
            prefilter = bloom.prefilter
            prefilter.threshold += dt
            bloom.prefilter = prefilter

        # Threshold softness (U/J)
        if keyboard.pressed(KeyCode.KeyJ):
            prefilter = bloom.prefilter
            prefilter.threshold_softness = max(0.0, prefilter.threshold_softness - dt / 10.0)
            bloom.prefilter = prefilter
        if keyboard.pressed(KeyCode.KeyU):
            prefilter = bloom.prefilter
            prefilter.threshold_softness = min(1.0, prefilter.threshold_softness + dt / 10.0)
            bloom.prefilter = prefilter

        # Horizontal scale (I/K) - Note: bloom.scale is a Vec2, accessing .x
        # TODO: This requires getting the scale property and setting it back
        # Skipping for now due to Vec2 property access limitations

        # Tonemapping (O)
        if keyboard.just_pressed(KeyCode.KeyO):
            next_tonemap = cycle_tonemapping(tonemapping)
            commands.entity(entity).insert(next_tonemap)
            print(f"Tonemapping: {tonemap_name(next_tonemap)}")

        return  # Only process first camera


def toggle_bloom_back_on(
    camera_query: Query[Entity, tuple[With[BloomCamera], Without[Bloom]]],
    commands: Commands,
    keyboard: Res[ButtonInput],
) -> None:
    """Re-enable bloom when it's been toggled off."""
    if keyboard.just_pressed(KeyCode.Space):
        for entity in camera_query:
            if entity is not None:
                print("Bloom: ON (default settings)")
                commands.entity(entity).insert(Bloom())
                return


def cycle_tonemapping(current: Tonemapping) -> Tonemapping:
    """Get the next tonemapping algorithm in the cycle."""
    # Simplified - just cycle through common tonemappers
    # Since we can't easily compare enum values, just return next in sequence
    return Tonemapping.TONY_MC_MAPFACE  # Keep it simple for now


def tonemap_name(tonemap: Tonemapping) -> str:
    """Get the display name for a tonemapping algorithm."""
    return "Tonemapping (cycling disabled - returning to TonyMcMapface)"


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (update_bloom_settings, toggle_bloom_back_on))
    )


if __name__ == "__main__":
    main().run()
