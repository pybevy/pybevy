"""Demonstrates using key modifiers (Ctrl, Shift, Alt).

Demonstrates:
- Checking multiple modifier keys at once
- Using any_pressed() to check left/right variants
- Combining modifiers with regular keys
- Ctrl + Shift + A hotkey detection

This system prints when Ctrl + Shift + A is pressed.
"""

from pybevy.prelude import *


def keyboard_input_system(input: Res[ButtonInput]) -> None:
    """Detect Ctrl + Shift + A key combination."""
    shift = input.any_pressed([KeyCode.ShiftLeft, KeyCode.ShiftRight])
    ctrl = input.any_pressed([KeyCode.ControlLeft, KeyCode.ControlRight])

    if ctrl and shift and input.just_pressed(KeyCode.KeyA):
        print("Just pressed Ctrl + Shift + A!")


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Update, keyboard_input_system)


if __name__ == "__main__":
    main().run()
