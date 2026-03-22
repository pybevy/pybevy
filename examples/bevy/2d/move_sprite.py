"""Renders a 2D scene containing a single, moving sprite.

Demonstrates:
- Sprite movement using Transform
- Time-based movement for consistent speed
- Enum components for state management
- Direction change based on position
"""

from dataclasses import dataclass

from pybevy.prelude import *


@component
@dataclass
class Direction(Component):
    """Component representing sprite movement direction."""

    LEFT: int = 0
    RIGHT: int = 1


def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    """Spawn camera and moving sprite."""
    commands.spawn(Camera2d())

    image = asset_server.load_image("icon.png")
    direction = Direction()
    direction.RIGHT = True
    direction.LEFT = False
    commands.spawn(
        Sprite.from_image(image),
        Transform.from_xyz(0.0, 0.0, 0.0),
        direction,
    )


def sprite_movement(
    time: Res[Time],
    sprite_query: Query[tuple[Mut[Direction], Mut[Transform]]],
) -> None:
    """Move the sprite left and right based on its direction.

    The sprite is animated by changing its translation depending on the time
    that has passed since the last frame.
    """
    for direction, transform in sprite_query:
        # Move based on direction
        if direction.RIGHT:
            transform.translation.x += 150.0 * time.delta_secs()
        else:  # Direction.LEFT
            transform.translation.x -= 150.0 * time.delta_secs()

        # Change direction at boundaries
        if transform.translation.x > 200.0:
            direction.LEFT = True
            direction.RIGHT = False
        elif transform.translation.x < -200.0:
            direction.LEFT = False
            direction.RIGHT = True


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, sprite_movement)
    )


if __name__ == "__main__":
    main().run()
