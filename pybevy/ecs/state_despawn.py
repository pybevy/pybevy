"""Systems for automatically despawning entities on state transitions.

This module provides systems that handle DespawnOnExit and DespawnOnEnter components.
"""

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from . import (
        Commands,
        DespawnOnEnter,
        DespawnOnExit,
        Entity,
        Local,
        Query,
        Res,
        State,
    )
else:
    from . import (
        Commands,
        DespawnOnEnter,
        DespawnOnExit,
        Entity,
        Local,
        Query,
        Res,
        State,
    )


class _PreviousState:
    """Wrapper for tracking previous state value in Local."""

    def __init__(self) -> None:
        self.value: object = None


def despawn_on_state_change(
    commands: Commands,
    current_state: Res[State],
    previous_state: Local[_PreviousState],
    despawn_on_exit: "Query[tuple[Entity, DespawnOnExit]]",
    despawn_on_enter: "Query[tuple[Entity, DespawnOnEnter]]",
) -> None:
    """System that despawns entities when states change.

    This system tracks state transitions and despawns entities with:
    - DespawnOnExit(state): Despawned when exiting the specified state
    - DespawnOnEnter(state): Despawned when entering the specified state

    Add this system to Update to enable automatic entity despawning:
        from pybevy.ecs.state_despawn import despawn_on_state_change
        app.add_systems(Update, despawn_on_state_change)
    """
    current = current_state.get()

    # First run - just store the current state
    if previous_state.value is None:
        previous_state.value = current
        return

    prev = previous_state.value

    if prev == current:
        return

    old_state = prev
    new_state = current

    # Despawn entities that should exit with the old state
    for entity, exit_comp in despawn_on_exit:
        if exit_comp.state_value() == old_state:
            commands.entity(entity).despawn()

    # Despawn entities that should despawn when entering the new state
    for entity, enter_comp in despawn_on_enter:
        if enter_comp.state_value() == new_state:
            commands.entity(entity).despawn()

    # Update previous state
    previous_state.value = current
