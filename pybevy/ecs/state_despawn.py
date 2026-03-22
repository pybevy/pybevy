"""Systems for automatically despawning entities on state transitions.

This module provides systems that handle DespawnOnExit and DespawnOnEnter components.
"""

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from pybevy.ecs import (
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
    from pybevy.ecs import (
        Commands,
        DespawnOnEnter,
        DespawnOnExit,
        Entity,
        Local,
        Query,
        Res,
        State,
    )


def despawn_on_state_change(
    commands: Commands,
    current_state: Res[State],
    previous_state: Local[list],  # Stores [previous_state_value] or empty list initially
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
    # Get the current state enum value
    # Note: Res[State].get() forwards to State.get() which returns the enum value
    current = current_state.get()

    # First run - just store the current state
    if len(previous_state.value) == 0:
        previous_state.value.append(current)
        return

    prev = previous_state.value[0]

    # Check if state changed using equality comparison
    # Note: We use == instead of 'is' because enum values may be different object
    # instances even when they represent the same enum member (due to PyO3 reference handling)
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
    previous_state.value[0] = current
