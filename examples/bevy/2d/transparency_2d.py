"""Demonstrates how to use transparency in 2D.

Shows 3 bevy logos on top of each other, each with a different amount of transparency.

Demonstrates:
- Sprite color property for transparency
- Alpha channel control with Color.srgba()
- Z-ordering with Transform
- Asset handle cloning
"""

from pybevy.prelude import *


def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    """Spawn camera and three overlapping sprites with different transparency."""
    commands.spawn(Camera2d())

    sprite_handle = asset_server.load_image("icon.png")

    # First sprite: fully opaque
    commands.spawn(
        Sprite.from_image(sprite_handle),
        Transform.from_xyz(-100.0, 0.0, 0.0),
    )

    # Second sprite: blue tint with 70% opacity
    commands.spawn(
        Sprite(
            image=sprite_handle,
            color=Color.srgba(0.0, 0.0, 1.0, 0.7),  # Alpha channel controls transparency
        ),
        Transform.from_xyz(0.0, 0.0, 0.1),
    )

    # Third sprite: green tint with 30% opacity
    commands.spawn(
        Sprite(
            image=sprite_handle,
            color=Color.srgba(0.0, 1.0, 0.0, 0.3),
        ),
        Transform.from_xyz(100.0, 0.0, 0.2),
    )


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)


if __name__ == "__main__":
    main().run()
