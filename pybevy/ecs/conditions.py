"""Common condition helpers for use with run_if().

This module provides helper functions for creating common conditions.

Note: These are Python-side helpers that work with the current run_if() implementation.
They create simple condition functions that can be used directly.
"""

from collections.abc import Callable

__all__ = ["always", "and_", "never", "not_", "or_"]


def always() -> Callable[[], bool]:
    """A condition that always returns True.

    Useful for testing or as a placeholder.

    Returns:
        A condition function that always returns True

    Example:
        ```python
        from pybevy.ecs.conditions import always
        from pybevy.ecs import run_if

        app.add_systems(Update, run_if(my_system, always()))
        ```
    """

    def condition() -> bool:
        return True

    return condition


def never() -> Callable[[], bool]:
    """A condition that always returns False.

    Useful for temporarily disabling systems.

    Returns:
        A condition function that always returns False

    Example:
        ```python
        from pybevy.ecs.conditions import never
        from pybevy.ecs import run_if

        app.add_systems(Update, run_if(my_system, never()))
        ```
    """

    def condition() -> bool:
        return False

    return condition


def and_(*conditions: Callable[[], bool]) -> Callable[[], bool]:
    """Combines multiple conditions with AND logic.

    All conditions must return True for the combined condition to return True.

    Args:
        *conditions: Variable number of condition functions to combine

    Returns:
        A condition function that returns True only if all conditions return True

    Example:
        ```python
        from pybevy.ecs.conditions import and_
        from pybevy.ecs import run_if

        def condition1() -> bool:
            return True

        def condition2() -> bool:
            return True

        # System runs only when both conditions are True
        app.add_systems(Update, run_if(my_system, and_(condition1, condition2)))
        ```
    """

    def combined() -> bool:
        return all(c() for c in conditions)

    return combined


def or_(*conditions: Callable[[], bool]) -> Callable[[], bool]:
    """Combines multiple conditions with OR logic.

    At least one condition must return True for the combined condition to return True.

    Args:
        *conditions: Variable number of condition functions to combine

    Returns:
        A condition function that returns True if any condition returns True

    Example:
        ```python
        from pybevy.ecs.conditions import or_
        from pybevy.ecs import run_if

        def condition1() -> bool:
            return False

        def condition2() -> bool:
            return True

        # System runs when either condition is True
        app.add_systems(Update, run_if(my_system, or_(condition1, condition2)))
        ```
    """

    def combined() -> bool:
        return any(c() for c in conditions)

    return combined


def not_(condition: Callable[[], bool]) -> Callable[[], bool]:
    """Negates a condition.

    Returns True when the inner condition returns False, and vice versa.

    Args:
        condition: The condition function to negate

    Returns:
        A condition function that returns the opposite of the input condition

    Example:
        ```python
        from pybevy.ecs.conditions import not_
        from pybevy.ecs import run_if

        def is_paused() -> bool:
            return False

        # System runs when NOT paused
        app.add_systems(Update, run_if(my_system, not_(is_paused)))
        ```
    """

    def negated() -> bool:
        return not condition()

    return negated




def input_just_pressed(key_code):
    """
    Create a condition that checks if a keyboard key was just pressed.

    Args:
        key_code: The key code to check (e.g., KeyCode.Space)

    Returns:
        A condition function with proper type annotations

    Example:
        ```python
        from pybevy.input import KeyCode
        from pybevy.ecs.conditions import input_just_pressed

        app.add_systems(
            Update,
            run_if(jump_system, input_just_pressed(KeyCode.Space))
        )
        ```
    """
    from ..input import ButtonInput

    def condition(input_state: ButtonInput) -> bool:
        return input_state.just_pressed(key_code)

    condition.__name__ = f"input_just_pressed_{key_code}"
    return condition


def state_is_active(state_type: type, target_value: int):
    """
    Create a condition that checks if a resource's value matches a target.

    Args:
        state_type: The resource type to check
        target_value: The value to compare against

    Returns:
        A condition function with proper type annotations

    Example:
        ```python
        from . import Resource, Res
        from pybevy.ecs.conditions import state_is_active

        class GamePhase(Resource):
            phase: int = 0

        # Only run when phase == 2
        app.add_systems(
            Update,
            run_if(my_system, state_is_active(GamePhase, 2))
        )
        ```
    """
    from . import Res

    def condition(res: Res) -> bool:  # type: ignore
        return hasattr(res, 'value') and res.value == target_value

    condition.__name__ = f"state_is_{state_type.__name__}_{target_value}"
    return condition
