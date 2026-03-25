"""Demonstrates comprehensive window configuration with WindowPlugin.

Demonstrates:
- Window title customization
- Window resolution (size) configuration
- Window decorations on/off
- Resizable window setting
- WindowResolution for size control
- WindowPlugin.set() pattern for configuration
"""

from pybevy.prelude import *
from pybevy.window import WindowResolution


def setup(commands: Commands) -> None:
    """Set up a camera."""
    commands.spawn(Camera2d())
    print("Window opened with custom configuration!")
    print("- Title: 'PyBevy - Window Configuration Example'")
    print("- Size: 1024x768")
    print("- Resizable: Yes")
    print("- Decorations: Yes")


@entrypoint
def main(app: App) -> App:
    """Configure and return the app with custom window settings."""
    # Create a custom window configuration
    window = Window(
        title="PyBevy - Window Configuration Example",  # Custom window title
        resolution=WindowResolution(1024, 768),  # Window size (width, height)
        decorations=True,  # Show window title bar and borders
        resizable=True,  # Allow window to be resized by user
        transparent=False,  # Opaque window (default)
    )

    # Use WindowPlugin.set() to configure the primary window
    return (
        app.add_plugins(DefaultPlugins().set(WindowPlugin(primary_window=window)))
        .add_systems(Startup, setup)
    )


if __name__ == "__main__":
    main().run()
