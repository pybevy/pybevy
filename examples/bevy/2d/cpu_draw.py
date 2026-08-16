"""Example of how to draw to a texture from the CPU.

You can set the values of individual pixels to whatever you want.
PyBevy provides APIs that work with NumPy arrays for efficient pixel manipulation.
"""

import math
from dataclasses import dataclass, field

import numpy as np

from pybevy.prelude import *
from pybevy.render import Extent3d, TextureFormat

IMAGE_WIDTH = 256
IMAGE_HEIGHT = 256


@resource
class MyProcGenImage(Resource):
    """Store the image handle that we will draw to."""
    def __init__(self, handle: Handle):
        self.handle = handle


@dataclass
class IntWrapper:
    """Wrapper for int to use with Local."""
    value: int = 0


@dataclass
class ColorWrapper:
    """Wrapper for Color to use with Local."""
    value: Color = field(default_factory=lambda: Color.WHITE)


def setup(commands: Commands, images: ResMut[Assets[Image]]) -> None:
    """Set up the scene with a camera and a procedurally generated image."""
    commands.spawn(Camera2d())

    # Create an image that we are going to draw into
    beige = Color.srgb(0.96, 0.96, 0.86)  # CSS beige
    # Convert color to bytes (RGBA8)
    beige_srgba = beige.to_srgba()
    beige_bytes = [
        int(beige_srgba.red * 255),
        int(beige_srgba.green * 255),
        int(beige_srgba.blue * 255),
        int(beige_srgba.alpha * 255)
    ]

    image = Image.new_fill(
        Extent3d(IMAGE_WIDTH, IMAGE_HEIGHT, 1),
        beige_bytes,
        format=TextureFormat.Rgba8UnormSrgb,
    )

    # To make it extra fancy, we can set the Alpha of each pixel,
    # so that it fades out in a circular fashion.
    center = Vec2(IMAGE_WIDTH / 2.0, IMAGE_HEIGHT / 2.0)
    max_radius = min(IMAGE_HEIGHT, IMAGE_WIDTH) / 2.0

    for y in range(IMAGE_HEIGHT):
        for x in range(IMAGE_WIDTH):
            r = Vec2(float(x), float(y)).distance(center)
            a = 1.0 - min(r / max_radius, 1.0)

            # Set the alpha value by accessing the raw pixel bytes
            with image.pixel_bytes_mut(UVec3(x, y, 0)) as pixel_bytes:
                # Convert f32 to u8 (it's the 4th byte for RGBA)
                pixel_bytes[3] = int(a * 255)

    # Add it to Bevy's assets
    handle = images.add(image)

    # Create a sprite entity using our image
    commands.spawn(Sprite.from_image(handle))
    commands.insert_resource(MyProcGenImage(handle))


def draw(
    my_handle: Res[MyProcGenImage],
    images: ResMut[Assets[Image]],
    i: Local[IntWrapper],
    draw_color: Local[ColorWrapper],
) -> None:
    """Every fixed update tick, draw one more pixel to make a spiral pattern."""
    # Generate a random color on first run
    if i.value == 0:
        draw_color.value = Color.linear_rgb(
            float(np.random.random()),
            float(np.random.random()),
            float(np.random.random()),
        )

    # Get the image from Bevy's asset storage
    image = images.get_mut(my_handle.handle)
    if not image:
        return

    # Compute the position of the pixel to draw
    center = Vec2(IMAGE_WIDTH / 2.0, IMAGE_HEIGHT / 2.0)
    max_radius = min(IMAGE_HEIGHT, IMAGE_WIDTH) / 2.0
    rot_speed = 0.0123
    period = 0.12345

    r = math.sin(i.value * period) * max_radius
    angle = i.value * rot_speed
    xy = Vec2(math.cos(angle), math.sin(angle)) * r + center
    x, y = int(xy.x), int(xy.y)

    # Get the old color of that pixel
    old_color = image.get_color_at(x, y)
    if not old_color:
        i.value += 1
        return

    # Occasionally change drawing color (simplified from Bevy version)
    if i.value % 1000 == 0:
        draw_color.value = Color.linear_rgb(
            float(np.random.random()),
            float(np.random.random()),
            float(np.random.random()),
        )

    # Set the new color, but keep old alpha value from image
    new_color = draw_color.value.with_alpha(old_color.alpha())
    image.set_color_at(x, y, new_color)

    i.value += 1


@entrypoint
def main(app: App) -> App:
    """Configure and return the app."""
    # Note: FixedUpdate runs at 64 Hz by default in PyBevy
    # The Rust version uses 1024 Hz but PyBevy doesn't expose Time<Fixed>.from_hz() yet
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, draw)
    )


if __name__ == "__main__":
    main().run()
