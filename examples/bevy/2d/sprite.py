"""Displays a single Sprite, created from an image.

A minimal 2D sprite example showing:
- Loading an image asset
- Creating a sprite from an image
- Rendering with Camera2d
"""

from pybevy.assets import AssetServer
from pybevy.ecs import Commands, Res
from pybevy.prelude import *


def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    """Spawn camera and sprite."""
    commands.spawn(Camera2d())

    image_handle = asset_server.load_image("bevy/branding/bevy_bird_dark.png")
    commands.spawn(Sprite.from_image(image_handle))


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)


if __name__ == "__main__":
    main().run()
