"""Entrypoint discovery shared by host launchers."""

import weakref


def is_entrypoint(obj: object) -> bool:
    """Recognize the entrypoint marker."""
    marker = getattr(obj, "__pybevy_entrypoint_decorated__", None)
    return callable(obj) and isinstance(marker, weakref.ReferenceType) and marker() is obj
