"""Demonstrates keyboard input handling.

Demonstrates:
- ButtonInput resource for keyboard state
- Key press detection (pressed, just_pressed, just_released)
- KeyCode for key location-based input
"""

from pybevy.input import ButtonInput, KeyCode
from pybevy.prelude import *


def keyboard_system(keyboard: Res[ButtonInput]) -> None:
    """Check keyboard input state."""
    # Check if key is currently held down
    if keyboard.pressed(KeyCode.KeyW):
        print("W key is pressed")

    # Check if key was just pressed this frame
    if keyboard.just_pressed(KeyCode.Space):
        print("Space was just pressed!")

    # Check if key was just released this frame
    if keyboard.just_released(KeyCode.Escape):
        print("Escape was just released")

    # Arrow keys
    if keyboard.pressed(KeyCode.ArrowUp):
        print("Up arrow pressed")
    if keyboard.pressed(KeyCode.ArrowDown):
        print("Down arrow pressed")


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Update, keyboard_system)


if __name__ == "__main__":
    main().run()
