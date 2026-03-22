"""Prints out all character inputs as they are typed.

Demonstrates:
- Character input events via MessageReader
- Filtering for pressed state only
- Using get_character() to extract character input
- Distinguishing character input from control keys

This system prints all character events as they come in (a, b, c, 1, 2, 3, etc.)
but filters out control keys (Escape, Backspace, Arrow keys, etc.).
"""

from pybevy.input import ButtonState, KeyboardInput
from pybevy.prelude import *


def print_char_event_system(keyboard_inputs: MessageReader[KeyboardInput]) -> None:
    """Print all character inputs as they come in."""
    for event in keyboard_inputs:
        # Only check for characters when the key is pressed
        if event.state != ButtonState.Pressed():
            continue

        # Get the character (filters out control keys automatically)
        if character := event.text:
            print(f"{event}: '{character}'")


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Update, print_char_event_system)


if __name__ == "__main__":
    main().run()
