"""
ViewColumn field accessors for structured types.

This module provides Python wrapper classes that add field access to ViewColumn handles.
For example, `TransformViewColumn` provides `.translation`, `.rotation`, `.scale` properties,
and `Vec3ViewColumn` provides `.x`, `.y`, `.z` properties.

These wrappers enable natural syntax like:
    transform = batch.column_mut(Transform)
    transform.translation.y  # Returns ViewColumn for y component
    transform.translation.y = (transform.translation.x * 0.5).sin()  # Assignment
"""

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from pybevy.ecs import ViewColumn


class Vec3ViewColumn:
    """Wrapper for ViewColumn pointing to a Vec3 field.

    Provides `.x`, `.y`, `.z` properties that return ViewColumn handles
    for individual components. Supports assignment via `__setattr__`.
    """

    _col: "ViewColumn"

    def __init__(self, view_column: "ViewColumn") -> None:
        object.__setattr__(self, "_col", view_column)

    @property
    def x(self) -> "ViewColumn":
        """ViewColumn for the X component (offset 0)."""
        return self._col.at_offset(0, "f4")

    @property
    def y(self) -> "ViewColumn":
        """ViewColumn for the Y component (offset 4)."""
        return self._col.at_offset(4, "f4")

    @property
    def z(self) -> "ViewColumn":
        """ViewColumn for the Z component (offset 8)."""
        return self._col.at_offset(8, "f4")

    def __setattr__(self, name: str, value: "ViewColumn | float") -> None:
        if name in ("x", "y", "z"):
            target: ViewColumn = getattr(self, name)
            target.set(value)
        else:
            object.__setattr__(self, name, value)  # type: ignore[assignment]


class QuatViewColumn:
    """Wrapper for ViewColumn pointing to a Quat field.

    Provides `.x`, `.y`, `.z`, `.w` properties that return ViewColumn handles
    for individual quaternion components. Supports assignment via `__setattr__`.
    """

    _col: "ViewColumn"

    def __init__(self, view_column: "ViewColumn") -> None:
        object.__setattr__(self, "_col", view_column)

    @property
    def x(self) -> "ViewColumn":
        """ViewColumn for the X component (offset 0)."""
        return self._col.at_offset(0, "f4")

    @property
    def y(self) -> "ViewColumn":
        """ViewColumn for the Y component (offset 4)."""
        return self._col.at_offset(4, "f4")

    @property
    def z(self) -> "ViewColumn":
        """ViewColumn for the Z component (offset 8)."""
        return self._col.at_offset(8, "f4")

    @property
    def w(self) -> "ViewColumn":
        """ViewColumn for the W component (offset 12)."""
        return self._col.at_offset(12, "f4")

    def __setattr__(self, name: str, value: "ViewColumn | float") -> None:
        if name in ("x", "y", "z", "w"):
            target: ViewColumn = getattr(self, name)
            target.set(value)
        else:
            object.__setattr__(self, name, value)  # type: ignore[assignment]


class Vec2ViewColumn:
    """Wrapper for ViewColumn pointing to a Vec2 field.

    Provides `.x`, `.y` properties that return ViewColumn handles
    for individual components. Supports assignment via `__setattr__`.
    """

    _col: "ViewColumn"

    def __init__(self, view_column: "ViewColumn") -> None:
        object.__setattr__(self, "_col", view_column)

    @property
    def x(self) -> "ViewColumn":
        """ViewColumn for the X component (offset 0)."""
        return self._col.at_offset(0, "f4")

    @property
    def y(self) -> "ViewColumn":
        """ViewColumn for the Y component (offset 4)."""
        return self._col.at_offset(4, "f4")

    def __setattr__(self, name: str, value: "ViewColumn | float") -> None:
        if name in ("x", "y"):
            target: ViewColumn = getattr(self, name)
            target.set(value)
        else:
            object.__setattr__(self, name, value)  # type: ignore[assignment]


class TransformViewColumn:
    """Wrapper for ViewColumn pointing to a Transform component.

    Provides `.translation`, `.rotation`, `.scale` properties that return
    wrapper objects for Vec3/Quat field access.

    Example:
        transform = batch.column_mut(Transform)
        y_positions = transform.translation.y  # ViewColumn for y coords
        transform.translation.y = (transform.translation.x * 0.5).sin()
    """

    _col: "ViewColumn"

    def __init__(self, view_column: "ViewColumn") -> None:
        object.__setattr__(self, "_col", view_column)

    @property
    def rotation(self) -> QuatViewColumn:
        """Access rotation field (Quat, offset 0)."""
        return QuatViewColumn(self._col.at_offset(0, "struct"))

    @property
    def translation(self) -> Vec3ViewColumn:
        """Access translation field (Vec3, offset 16)."""
        return Vec3ViewColumn(self._col.at_offset(16, "struct"))

    @property
    def scale(self) -> Vec3ViewColumn:
        """Access scale field (Vec3, offset 28)."""
        return Vec3ViewColumn(self._col.at_offset(28, "struct"))

    def __setattr__(self, name: str, value: "ViewColumn | float") -> None:
        if name in ("rotation", "translation", "scale"):
            raise TypeError(
                f"Cannot assign directly to '{name}'. "
                f"Assign to individual fields instead, e.g. "
                f"transform.{name}.x = value"
            )
        object.__setattr__(self, name, value)  # type: ignore[assignment]


__all__ = ["QuatViewColumn", "TransformViewColumn", "Vec2ViewColumn", "Vec3ViewColumn"]
