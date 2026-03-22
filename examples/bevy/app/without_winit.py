"""Create an application without winit (runs single time, no event loop).

Demonstrates:
- Disabling WinitPlugin to run without a window
- Using PluginGroupBuilder.disable() method
- Running a headless application
"""

from pybevy.app import App, DefaultPlugins
from pybevy.camera import Camera3d
from pybevy.decorators import entrypoint
from pybevy.ecs import Commands
from pybevy.prelude import Update
from pybevy.winit import WinitPlugin


def setup_system(commands: Commands) -> None:
    """Spawn a camera (even though no window will display it)."""
    commands.spawn(Camera3d())


@entrypoint
def main(app: App) -> App:
    """Configure app without winit window system."""
    return (
        app.add_plugins(DefaultPlugins().build().disable(WinitPlugin))
        .add_systems(Update, setup_system)
    )


if __name__ == "__main__":
    main().run()
