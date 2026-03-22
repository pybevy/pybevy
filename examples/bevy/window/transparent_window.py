"""Shows how to display a window in transparent mode.

This feature works as expected depending on the platform. Please check the Bevy
documentation for more details on platform-specific behavior.

Note: PyBevy currently doesn't expose CompositeAlphaMode, so platform-specific
alpha compositing settings from the Rust version are not included.
"""

from pybevy.app import App, DefaultPlugins
from pybevy.assets import AssetServer
from pybevy.camera import Camera2d, ClearColor
from pybevy.color import Color
from pybevy.decorators import entrypoint
from pybevy.ecs import Commands, Res
from pybevy.prelude import Startup
from pybevy.sprite import Sprite
from pybevy.window import Window, WindowPlugin


def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    """Set up a camera and sprite."""
    commands.spawn(Camera2d())
    commands.spawn(Sprite.from_image(asset_server.load_image("icon.png")))


@entrypoint
def main(app: App) -> App:
    """Configure and return the app with a transparent window."""
    # Create a transparent window without decorations
    window = Window(
        transparent=True,  # Allow ClearColor's alpha to take effect
        decorations=False,  # Remove window decorations for widget-like appearance
    )

    return (
        app.add_plugins(DefaultPlugins().set(WindowPlugin(primary_window=window)))
        # ClearColor must have 0 alpha, otherwise some color will bleed through
        .insert_resource(ClearColor(Color.NONE))
        .add_systems(Startup, setup)
    )


if __name__ == "__main__":
    main().run()
