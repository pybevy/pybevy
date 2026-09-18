"""Common condition helpers for use with run_if().

This module provides helper functions for creating common conditions.

Note: These are Python-side helpers that work with the current run_if() implementation.
They create simple condition functions that can be used directly.
"""

from __future__ import annotations

from collections.abc import Callable
from enum import Enum
from typing import TypeVar

from ..input import ButtonInput, KeyCode
from . import Res, State, in_state

__all__ = [
    "always",
    "and_",
    "input_just_pressed",
    "never",
    "not_",
    "or_",
    "state_is_active",
]

StateT = TypeVar("StateT", bound=Enum)


def always() -> Callable[[], bool]:
    """Build a condition that always returns True.

    Call this factory before passing its result to `run_if`.

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
    """Build a condition that always returns False.

    Call this factory before passing its result to `run_if`.

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


def and_(*conditions: Callable[..., bool]) -> Callable[..., bool]:
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

    combined.__pybevy_condition_expr__ = ("and", conditions)  # type: ignore[attr-defined]
    return combined


def or_(*conditions: Callable[..., bool]) -> Callable[..., bool]:
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

    combined.__pybevy_condition_expr__ = ("or", conditions)  # type: ignore[attr-defined]
    return combined


def not_(condition: Callable[..., bool]) -> Callable[..., bool]:
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

    negated.__pybevy_condition_expr__ = (  # type: ignore[attr-defined]
        "not",
        (condition,),
    )
    return negated


def input_just_pressed(
    key_code: KeyCode,
) -> Callable[[Res[ButtonInput[KeyCode]]], bool]:
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
    def condition(input_state: Res[ButtonInput[KeyCode]]) -> bool:
        return input_state.just_pressed(key_code)

    condition.__name__ = f"input_just_pressed_{key_code}"
    return condition


def state_is_active(
    state_type: type[StateT], target_value: StateT
) -> Callable[[Res[State[StateT]]], bool]:
    """
    Create a condition that checks if a state machine has the target state.

    Args:
        state_type: The ``@state`` enum type to check
        target_value: The enum member to compare against

    Returns:
        A condition function with proper type annotations

    Example:
        ```python
        from enum import Enum, auto

        from pybevy.ecs import state
        from pybevy.ecs.conditions import state_is_active

        @state
        class GamePhase(Enum):
            MENU = auto()
            PLAYING = auto()

        # Only run while the game is playing
        app.add_systems(
            Update,
            run_if(my_system, state_is_active(GamePhase, GamePhase.PLAYING))
        )
        ```
    """
    if not isinstance(target_value, state_type):
        raise TypeError(
            f"target_value must be a member of {state_type.__name__}, "
            f"got {type(target_value).__name__}"
        )

    condition = in_state(target_value)
    condition.__name__ = f"state_is_{state_type.__name__}_{target_value}"
    return condition
