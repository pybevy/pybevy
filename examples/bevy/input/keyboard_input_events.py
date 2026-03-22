"""Prints out all keyboard events as they occur.

Demonstrates using MessageReader to receive keyboard input events.
Shows the difference between:
- ButtonInput<KeyCode>: Current state of keys (is it pressed right now?)
- KeyboardInput messages: Stream of events (key was pressed/released)
"""

from pybevy.input import KeyboardInput
from pybevy.prelude import *


def print_keyboard_event_system(keyboard_inputs: MessageReader[KeyboardInput]) -> None:
    """System that prints all keyboard inputs as they come in."""
    for event in keyboard_inputs:
        print(f"Keyboard event: {event}")


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(
        Update, print_keyboard_event_system
    )


if __name__ == "__main__":
    main().run()
