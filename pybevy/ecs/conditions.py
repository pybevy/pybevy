"""Common run conditions and condition combinators for ``run_if()``.

Every helper returns a ``SystemCondition``, the object ``@condition`` produces.
"""

from __future__ import annotations

from enum import Enum
from typing import TypeVar, cast

from ..input import ButtonInput, KeyCode
from ..state import State, in_state
from . import Res, SystemCondition, condition
from ._callables import combine

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


def always() -> SystemCondition[[], bool]:
    """Build a condition that always returns True.

    Call this factory before passing its result to `run_if`.

    Returns:
        A condition that always returns True

    Example:
        ```python
        from pybevy.ecs.conditions import always

        app.add_systems(Update, my_system.run_if(always()))
        ```
    """

    @condition
    def always_true() -> bool:
        return True

    return always_true


def never() -> SystemCondition[[], bool]:
    """Build a condition that always returns False.

    Call this factory before passing its result to `run_if`.

    Returns:
        A condition that always returns False

    Example:
        ```python
        from pybevy.ecs.conditions import never

        app.add_systems(Update, my_system.run_if(never()))
        ```
    """

    @condition
    def always_false() -> bool:
        return False

    return always_false


def and_(*conditions: SystemCondition[..., object]) -> SystemCondition[..., bool]:
    """Combines multiple conditions with AND logic.

    All conditions must return True for the combined condition to return True.

    Args:
        *conditions: Variable number of condition functions to combine

    Returns:
        A condition that returns True only if all conditions return True

    Example:
        ```python
        from pybevy.ecs import condition
        from pybevy.ecs.conditions import and_

        @condition
        def condition1() -> bool:
            return True

        @condition
        def condition2() -> bool:
            return True

        # System runs only when both conditions are True
        app.add_systems(Update, my_system.run_if(and_(condition1, condition2)))
        ```
    """
    return cast("SystemCondition[..., bool]", combine("and", conditions, "and_()"))


def or_(*conditions: SystemCondition[..., object]) -> SystemCondition[..., bool]:
    """Combines multiple conditions with OR logic.

    At least one condition must return True for the combined condition to return True.

    Args:
        *conditions: Variable number of condition functions to combine

    Returns:
        A condition that returns True if any condition returns True

    Example:
        ```python
        from pybevy.ecs import condition
        from pybevy.ecs.conditions import or_

        @condition
        def condition1() -> bool:
            return False

        @condition
        def condition2() -> bool:
            return True

        # System runs when either condition is True
        app.add_systems(Update, my_system.run_if(or_(condition1, condition2)))
        ```
    """
    return cast("SystemCondition[..., bool]", combine("or", conditions, "or_()"))


def not_(inner: SystemCondition[..., object]) -> SystemCondition[..., bool]:
    """Negates a condition.

    Returns True when the inner condition returns False, and vice versa.

    Args:
        inner: The condition to negate

    Returns:
        A condition that returns the opposite of the input condition

    Example:
        ```python
        from pybevy.ecs import condition
        from pybevy.ecs.conditions import not_

        @condition
        def is_paused() -> bool:
            return False

        # System runs when NOT paused
        app.add_systems(Update, my_system.run_if(not_(is_paused)))
        ```
    """
    return cast("SystemCondition[..., bool]", combine("not", (inner,), "not_()"))


def input_just_pressed(
    key_code: KeyCode,
) -> SystemCondition[[Res[ButtonInput[KeyCode]]], bool]:
    """
    Create a condition that checks if a keyboard key was just pressed.

    Args:
        key_code: The key code to check (e.g., KeyCode.Space)

    Returns:
        A condition with typed parameters

    Example:
        ```python
        from pybevy.input import KeyCode
        from pybevy.ecs.conditions import input_just_pressed

        app.add_systems(Update, jump_system.run_if(input_just_pressed(KeyCode.Space)))
        ```
    """

    def pressed(input_state: Res[ButtonInput[KeyCode]]) -> bool:
        return input_state.just_pressed(key_code)

    pressed.__name__ = f"input_just_pressed_{key_code}"
    return condition(pressed)


def state_is_active(
    state_type: type[StateT], target_value: StateT
) -> SystemCondition[[Res[State[StateT]]], bool]:
    """
    Create a condition that checks if a state machine has the target state.

    Args:
        state_type: The ``@state`` enum type to check
        target_value: The enum member to compare against

    Returns:
        A condition with typed parameters

    Example:
        ```python
        from enum import Enum, auto

        from pybevy.state import state
        from pybevy.ecs.conditions import state_is_active

        @state
        class GamePhase(Enum):
            MENU = auto()
            PLAYING = auto()

        # Only run while the game is playing
        app.add_systems(
            Update, my_system.run_if(state_is_active(GamePhase, GamePhase.PLAYING))
        )
        ```
    """
    if not isinstance(target_value, state_type):
        raise TypeError(
            f"target_value must be a member of {state_type.__name__}, "
            f"got {type(target_value).__name__}"
        )

    active = in_state(target_value)
    active.__name__ = f"state_is_{state_type.__name__}_{target_value}"
    return active
