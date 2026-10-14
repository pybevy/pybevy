"""Headless UI widgets for PyBevy.

The widgets carry behavior only; style them with ordinary UI components.
"""

from pybevy.ecs import Component

class Button(Component):
    """Headless button widget.

    While the pointer holds the button down, the button carries the
    `pybevy.ui.Pressed` marker. Add `pybevy.picking.Hovered` to track hover.
    `pybevy.ui.InteractionDisabled` stops the button from reacting.

    Example:
        ```python
        from pybevy.ecs import Commands, Has, Mut, Query, With, system
        from pybevy.picking import Hovered
        from pybevy.ui import BackgroundColor, Node, Pressed
        from pybevy.ui_widgets import Button
        from pybevy.color import Color

        @system
        def setup(commands: Commands) -> None:
            commands.spawn(Button(), Hovered(), Node(), BackgroundColor(Color.BLACK))

        @system
        def style(
            buttons: Query[tuple[Hovered, Has[Pressed], Mut[BackgroundColor]], With[Button]],
        ) -> None:
            for hovered, pressed, color in buttons:
                if pressed:
                    color.color = Color.srgb(0.35, 0.75, 0.35)
                elif hovered.get():
                    color.color = Color.srgb(0.25, 0.25, 0.25)
                else:
                    color.color = Color.srgb(0.15, 0.15, 0.15)
        ```
    """

    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class TextInput(Component):
    """Editable text input widget.

    Pair it with `pybevy.text.EditableText`, which holds the text being edited:
    keyboard and pointer editing apply only to entities that carry both.

    Example:
        ```python
        from pybevy.text import EditableText
        from pybevy.ui import Node
        from pybevy.ui_widgets import TextInput

        commands.spawn(Node(), EditableText("hello"), TextInput())
        ```
    """

    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...

__all__ = ["Button", "TextInput"]
