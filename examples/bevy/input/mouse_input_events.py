"""Prints all mouse events to the console.

Demonstrates using MessageReader to receive various mouse input events:
- Mouse button presses/releases
- Mouse motion (delta movement)
- Cursor position changes
- Mouse wheel scrolling

Note: Gesture events (pinch, rotation, double-tap) are not yet implemented in PyBevy.
"""

from pybevy.input import MouseButtonInput, MouseMotion, MouseWheel
from pybevy.prelude import *


def print_mouse_events_system(
    mouse_button_input_reader: MessageReader[MouseButtonInput],
    mouse_motion_reader: MessageReader[MouseMotion],
    cursor_moved_reader: MessageReader[CursorMoved],
    mouse_wheel_reader: MessageReader[MouseWheel],
) -> None:
    """System that prints all mouse events as they come in."""
    for button_event in mouse_button_input_reader:
        print(f"Mouse button: {button_event}")

    for motion_event in mouse_motion_reader:
        print(f"Mouse motion (delta): {motion_event}")

    for cursor_event in cursor_moved_reader:
        print(f"Cursor moved (position): {cursor_event}")

    for wheel_event in mouse_wheel_reader:
        print(f"Mouse wheel: {wheel_event}")

    # Note: Gesture events (macOS specific) not yet implemented:
    # - PinchGesture
    # - RotationGesture
    # - DoubleTapGesture


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(
        Update, print_mouse_events_system
    )


if __name__ == "__main__":
    main().run()
