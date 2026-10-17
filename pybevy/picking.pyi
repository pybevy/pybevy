"""Picking components for PyBevy."""

from pybevy.ecs import Component

class Hovered(Component):
    """Whether the pointer hovers the entity or any of its descendants.

    Insert it on an entity to receive hover updates. The picking systems
    replace the value only when the pointer enters or leaves, so
    `Changed[Hovered]` reports hover transitions. The component is
    immutable: replace it rather than mutating it.
    """

    def __init__(self, value: bool = False) -> None: ...
    @property
    def value(self) -> bool:
        """True while the entity or a descendant is hovered."""

    def get(self) -> bool:
        """Return whether the entity is currently hovered."""

    def __copy__(self) -> Hovered: ...

    def __deepcopy__(self, memo: dict[int, object]) -> Hovered: ...
    def __eq__(self, other: object) -> bool: ...

__all__ = ["Hovered"]
