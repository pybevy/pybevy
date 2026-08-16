"""Renders a lot of sprites to allow performance testing.

This example sets up many sprites in different sizes, rotations, and scales in the world.
It also moves the camera over them to see how well frustum culling works.

Demonstrates:
- Spawning large numbers of sprites (102,400 by default)
- Sprite batching performance
- Camera movement and rotation
- Frustum culling effectiveness
- Timer-based periodic logging
- Local system state with Local<T>

Performance tips:
- Run with --release build for realistic performance measurement
- Observe frame time and sprite count in console output
- Camera moves through sprite field to test culling
"""

import random
import sys

from pybevy.prelude import *

CAMERA_SPEED = 1000.0

COLORS = [Color.srgb(0.0, 0.0, 1.0), Color.WHITE, Color.srgb(1.0, 0.0, 0.0)]


@resource
class ColorTint(Resource):
    """Controls whether sprites are color tinted (multiple batches) or not."""

    def __init__(self, enabled: bool):
        self.enabled = enabled


@resource
class PrintingTimer(Resource):
    """Timer for periodic sprite count logging."""

    def __init__(self):
        self.timer = Timer(1.0, TimerMode.Repeating)


def setup(
    commands: Commands, asset_server: Res[AssetServer], color_tint: Res[ColorTint]
) -> None:
    """Set up the camera and spawn many sprites in a grid."""
    tile_size = Vec2(64.0, 64.0)
    map_size = Vec2(320.0, 320.0)

    half_x = int(map_size.x / 2.0)
    half_y = int(map_size.y / 2.0)

    sprite_handle = asset_server.load_image("icon.png")

    # Spawn camera
    commands.spawn(Camera2d())

    # Build and spawn sprites
    # Note: Using loop instead of spawn_batch (not available in PyBevy)
    for y in range(-half_y, half_y):
        for x in range(-half_x, half_x):
            position = Vec2(float(x), float(y))
            translation_2d = position * tile_size
            translation = Vec3(
                translation_2d.x, translation_2d.y, random.random()
            )  # z for layering

            rotation = Quat.from_rotation_z(random.random())
            scale = Vec3.splat(random.random() * 2.0)

            sprite = Sprite(image=sprite_handle)
            sprite.custom_size = Vec2(tile_size.x, tile_size.y)

            if color_tint.enabled:
                sprite.color = COLORS[random.randint(0, 2)]
            else:
                sprite.color = Color.WHITE

            transform = Transform()
            transform.translation = translation
            transform.rotation = rotation
            transform.scale = scale

            commands.spawn(sprite, transform)

    print(
        f"Spawned {(half_x * 2) * (half_y * 2)} sprites. "
        "Camera will rotate and move through the field."
    )


def move_camera(
    time: Res[Time], camera_query: Single[Mut[Transform], With[Camera2d]]
) -> None:
    """Rotate and translate the camera to test frustum culling."""
    for camera_transform in camera_query:
        # Rotate around Z axis
        camera_transform.rotate_z(time.delta_secs() * 0.5)

        # Move forward in camera's current facing direction
        # Rotate the X axis vector by camera's rotation to get forward direction
        move_delta = Vec3(CAMERA_SPEED * time.delta_secs(), 0.0, 0.0)
        rotated_delta = camera_transform.rotation * move_delta  # type: ignore

        camera_transform.translation.x += rotated_delta.x  # type: ignore
        camera_transform.translation.y += rotated_delta.y  # type: ignore
        camera_transform.translation.z += rotated_delta.z  # type: ignore


def print_sprite_count(
    time: Res[Time], timer: ResMut[PrintingTimer], sprites: Query[Sprite]
) -> None:
    """Print the number of sprites every second."""
    timer.timer.tick(time.delta())

    if timer.timer.just_finished():
        count = sum(1 for _ in sprites)
        print(f"Sprites: {count}")


@entrypoint
def main(app: App) -> App:
    # Check for --colored command line argument
    color_tint = "--colored" in sys.argv

    if color_tint:
        print("Running with colored sprites (reduced batching performance)")

    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(ColorTint(color_tint))
        .insert_resource(PrintingTimer())
        .add_systems(Startup, setup)
        .add_systems(Update, (print_sprite_count, move_camera))
    )


if __name__ == "__main__":
    main().run()
