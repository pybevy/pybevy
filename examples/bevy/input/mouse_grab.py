"""Demonstrates how to grab and hide the mouse cursor.

Demonstrates:
- Querying CursorOptions component from window
- Setting cursor visibility
- Cursor grab modes (None, Confined, Locked)
- Mouse button input handling
- Typical FPS-style mouse capture

This system grabs the mouse when the left mouse button is pressed
and releases it when the escape key is pressed.
"""

from pybevy.input import ButtonInput, MouseButton
from pybevy.prelude import *
from pybevy.window import CursorGrabMode, CursorOptions


def grab_mouse(
    cursor_options_query: Single[Mut[CursorOptions]],
    mouse: Res[ButtonInput[MouseButton]],
    key: Res[ButtonInput[KeyCode]],
) -> None:
    """Grab mouse on left click, release on Escape."""
    cursor_options = cursor_options_query.into_inner()
    if mouse.just_pressed(MouseButton.Left()):
        cursor_options.visible = False
        cursor_options.grab_mode = CursorGrabMode.Locked

    if key.just_pressed(KeyCode.Escape):
        cursor_options.visible = True
        cursor_options.grab_mode = CursorGrabMode.None_


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Update, grab_mouse)


if __name__ == "__main__":
    main().run()
