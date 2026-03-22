"""Iterates and prints gamepad input and connection events.

Demonstrates:
- GamepadConnection events (connect/disconnect)
- GamepadAxisChanged events (continuous analog values)
- GamepadButtonChanged events (continuous button pressure)
- Using MessageReader for event-based gamepad input

Note: PyBevy's gamepad events don't include entity information (which gamepad
the event came from). For multi-gamepad support, use the gamepad_input.py
example which uses the Gamepad resource.
"""

from pybevy.input import (
    GamepadAxisChanged,
    GamepadButtonChanged,
    GamepadConnection,
)
from pybevy.prelude import *


def gamepad_events(
    connection_events: MessageReader[GamepadConnection],
    axis_changed_events: MessageReader[GamepadAxisChanged],
    button_changed_events: MessageReader[GamepadButtonChanged],
) -> None:
    """Print all gamepad events."""
    for connection_event in connection_events:
        if connection_event.connected:
            print("Gamepad connected")
        else:
            print("Gamepad disconnected")

    for axis_changed_event in axis_changed_events:
        print(f"{axis_changed_event.axis} is changed to {axis_changed_event.value}")

    for button_changed_event in button_changed_events:
        print(f"{button_changed_event.button} is changed to {button_changed_event.value}")


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Update, gamepad_events)


if __name__ == "__main__":
    main().run()
