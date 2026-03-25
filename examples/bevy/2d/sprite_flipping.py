"""Displays a single Sprite, created from an image, but flipped on one axis.

Demonstrates:
- Sprite flip_x and flip_y properties
- Sprite construction with specific parameters
"""

from pybevy.prelude import *


def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    """Spawn camera and flipped sprite."""
    commands.spawn(Camera2d())

    image_handle = asset_server.load_image("bevy/branding/bevy_bird_dark.png")
    commands.spawn(
        Sprite(
            image=image_handle,
            flip_x=True,  # Flip the logo to the left
            flip_y=False,  # Don't flip it upside-down (the default)
        )
    )


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)


if __name__ == "__main__":
    main().run()
