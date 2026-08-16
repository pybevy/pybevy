"""Prints mouse button events and motion.

Demonstrates:
- Mouse button state tracking (pressed, just_pressed, just_released)
- Mouse motion tracking with AccumulatedMouseMotion resource
- Mouse scroll tracking with MessageReader[MouseWheel]
- Multiple systems handling different input types

Note: PyBevy uses MessageReader[MouseWheel] for scroll events instead of
AccumulatedMouseScroll resource (which doesn't exist yet).
"""

from pybevy.input import AccumulatedMouseMotion, ButtonInput, MouseButton, MouseWheel
from pybevy.prelude import *


def mouse_click_system(mouse_button_input: Res[ButtonInput[MouseButton]]) -> None:
    """Print mouse button press/release events."""
    if mouse_button_input.pressed(MouseButton.Left()):
        print("left mouse currently pressed")

    if mouse_button_input.just_pressed(MouseButton.Left()):
        print("left mouse just pressed")

    if mouse_button_input.just_released(MouseButton.Left()):
        print("left mouse just released")


def mouse_move_system(
    accumulated_mouse_motion: Res[AccumulatedMouseMotion],
    mouse_scroll: MessageReader[MouseWheel],
) -> None:
    """Print mouse motion and scroll events."""
    # Check accumulated motion (resource-based)
    if accumulated_mouse_motion.delta.x != 0.0 or accumulated_mouse_motion.delta.y != 0.0:
        print(
            f"mouse moved ({accumulated_mouse_motion.delta.x}, {accumulated_mouse_motion.delta.y})"
        )

    # Check scroll events (message-based)
    for event in mouse_scroll:
        if event.x != 0.0 or event.y != 0.0:
            print(f"mouse scrolled ({event.x}, {event.y})")


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(
        Update, (mouse_click_system, mouse_move_system)
    )


if __name__ == "__main__":
    main().run()
