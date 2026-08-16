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
    cursor_options: Single[Mut[CursorOptions]],
    mouse: Res[ButtonInput[MouseButton]],
    key: Res[ButtonInput],
) -> None:
    """Grab mouse on left click, release on Escape."""
    for opts in cursor_options:
        if mouse.just_pressed(MouseButton.Left()):
            opts.visible = False
            opts.grab_mode = CursorGrabMode.Locked

        if key.just_pressed(KeyCode.Escape):
            opts.visible = True
            opts.grab_mode = CursorGrabMode.None_


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Update, grab_mouse)


if __name__ == "__main__":
    main().run()
