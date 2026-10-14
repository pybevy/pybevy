"""Keyboard and pointer focus for UI widgets, mirroring ``bevy::input_focus``.

``DefaultPlugins`` installs ``InputFocusPlugin`` and ``InputDispatchPlugin``, and a
primary click focuses the clicked entity when it carries a ``TabIndex``. Add
``TabNavigationPlugin`` for Tab and Shift+Tab navigation.
"""

from typing import ClassVar

import numpy as np

from pybevy.app import App, Plugin
from pybevy.ecs import Batchable, Component, Entity, Resource

class FocusCause:
    """Why an entity gained input focus."""

    def __copy__(self) -> FocusCause: ...
    def __deepcopy__(self, memo: dict[int, object]) -> FocusCause: ...

    Navigated: ClassVar[FocusCause]
    Pressed: ClassVar[FocusCause]
    Auto: ClassVar[FocusCause]

class TabIndex(Component):
    """Makes an entity focusable by click and, under a ``TabGroup``, by Tab navigation.

    Entities are visited in ascending ``value`` order within their group. A
    ``TextInput`` needs a ``TabIndex`` before a click can focus it.

    Example:
        ```python
        from pybevy.input_focus import TabIndex
        from pybevy.ui import Node
        from pybevy.ui_widgets import TextInput

        commands.spawn(Node(), TextInput(), TabIndex(0))
        ```
    """

    def __init__(self, value: int = 0) -> None: ...
    @property
    def value(self) -> int: ...
    @value.setter
    def value(self, value: int) -> None: ...
    @staticmethod
    def batch(*, value: np.typing.ArrayLike | None = None) -> Batchable: ...  # type: ignore[override]
    def __eq__(self, other: object) -> bool: ...

class TabGroup(Component):
    """Marks an entity tree as containing tabbable entities.

    Groups are visited in ascending ``order``. Tabbing inside a ``modal`` group
    cycles through that group only.
    """

    def __init__(self, order: int = 0, *, modal: bool = False) -> None: ...
    @property
    def order(self) -> int: ...
    @order.setter
    def order(self, value: int) -> None: ...
    @property
    def modal(self) -> bool: ...
    @modal.setter
    def modal(self, value: bool) -> None: ...
    @staticmethod
    def batch(  # type: ignore[override]
        *,
        order: np.typing.ArrayLike | None = None,
        modal: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class AutoFocus(Component):
    """Gives its entity input focus when the component is added."""

    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class InputFocus(Resource):
    """The entity that currently has input focus, if any.

    ``InputFocusPlugin`` (part of ``DefaultPlugins``) creates it. Prefer ``set``
    over inserting a new ``InputFocus``, which drops buffered focus changes.
    """

    def __init__(self) -> None: ...
    @staticmethod
    def from_entity(entity: Entity) -> InputFocus:
        """An ``InputFocus`` focused on ``entity``."""
    def get(self) -> Entity | None:
        """The focused entity, or ``None``."""
    def set(self, entity: Entity, cause: FocusCause) -> None:
        """Focus ``entity``. Requires ``ResMut[InputFocus]``."""
    def clear(self) -> None:
        """Clear input focus. Requires ``ResMut[InputFocus]``."""
    def __eq__(self, other: object) -> bool: ...

class InputFocusVisible(Resource):
    """Whether widgets should draw a focus indicator. Tab navigation sets it."""

    def __init__(self, value: bool = False) -> None: ...
    @property
    def value(self) -> bool: ...
    @value.setter
    def value(self, value: bool) -> None: ...

class InputFocusPlugin(Plugin):
    """Creates ``InputFocus`` and ``InputFocusVisible``; part of ``DefaultPlugins``."""

    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class InputDispatchPlugin(Plugin):
    """Dispatches keyboard, gamepad and mouse-wheel input to the focused entity; part of ``DefaultPlugins``."""

    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class TabNavigationPlugin(Plugin):
    """Moves input focus with Tab and Shift+Tab between ``TabIndex`` entities."""

    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...
