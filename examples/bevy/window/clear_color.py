"""Shows how to set the solid background color for the window.

The clear color acts as the background since pixels not drawn in a frame
remain unchanged.

Controls:
- Space: Change clear color to purple
"""

from pybevy.camera import ClearColor
from pybevy.ecs import Commands, Res, ResMut
from pybevy.input import ButtonInput, KeyCode
from pybevy.prelude import *


def setup(commands: Commands) -> None:
    """Spawn a 2D camera."""
    commands.spawn(Camera2d())


def change_clear_color(
    input: Res[ButtonInput], clear_color: ResMut[ClearColor]
) -> None:
    """Change the clear color when space is pressed."""
    if input.just_pressed(KeyCode.Space):
        # PURPLE is Color.srgb(0.5, 0.0, 0.5) from CSS palette
        clear_color.color = Color.srgb(0.5, 0.0, 0.5)


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(ClearColor(Color.srgb(0.5, 0.5, 0.9)))
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, change_clear_color)
    )


if __name__ == "__main__":
    main().run()
