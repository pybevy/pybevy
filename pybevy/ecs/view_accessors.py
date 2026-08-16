"""
ViewColumn field accessors for structured types.

This module provides Python wrapper classes that add field access to ViewColumn handles.
For example, `Vec3ViewColumn` provides `.x`, `.y`, `.z` properties. Transform
field dispatch itself is implemented by the native `ViewColumn`.

These wrappers enable natural syntax like:
    transform = batch.column_mut(Transform)
    transform.translation.y  # Returns ViewColumn for y component
    transform.translation.y = (transform.translation.x * 0.5).sin()  # Assignment
"""

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from . import ViewColumn


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

    def __getitem__(self, _index: object) -> None:
        raise TypeError(
            "Vec3ViewColumn indexing is not supported; select .x, .y, or .z "
            "first, then use integer indexing or to_numpy() on that ViewColumn"
        )


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

    def __getitem__(self, _index: object) -> None:
        raise TypeError(
            "QuatViewColumn indexing is not supported; select .x, .y, .z, or .w "
            "first, then use integer indexing or to_numpy() on that ViewColumn"
        )


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

    def __getitem__(self, _index: object) -> None:
        raise TypeError(
            "Vec2ViewColumn indexing is not supported; select .x or .y first, "
            "then use integer indexing or to_numpy() on that ViewColumn"
        )


__all__ = ["QuatViewColumn", "Vec2ViewColumn", "Vec3ViewColumn"]
