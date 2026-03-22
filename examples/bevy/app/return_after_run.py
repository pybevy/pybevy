"""Shows how to return to the calling function after a Bevy app has exited.

Demonstrates:
- App.run() returns control to the caller after the app exits
- Code execution continues after App.run()
- Using print statements before and after app.run()

Note: In windowed Bevy applications, executing code below App.run() is not
recommended because:
- App.run() will never return on iOS and Web
- It is not possible to recreate a window after the event loop has been terminated

This example uses DefaultPlugins and will create a window. Close the window to
see the "App has exited" message.

PyBevy limitation: WindowPlugin configuration (like setting window title) is not
yet available, so we use the default window configuration.
"""

from pybevy.prelude import *


def system() -> None:
    """System that runs each frame."""


@entrypoint
def main(app: App) -> App:
    print("Running Bevy App")
    return app.add_plugins(DefaultPlugins).add_systems(Update, system)


if __name__ == "__main__":
    main().run()
    print("Bevy App has exited. We are back in our main function.")
