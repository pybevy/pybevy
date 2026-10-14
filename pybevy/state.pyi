from typing import Generic, TypeVar

from pybevy.ecs import Component, Res, Resource, SystemCondition

StateType = TypeVar("StateType")

class State(Generic[StateType], Resource):
    """Current state resource.

    Holds the current state value for a state machine.
    Created automatically by app.init_state() or app.insert_state().

    Example:
        @system
        def check_state(current: Res[State[GameState]]) -> None:
            if current.get() == GameState.MENU:
                print("In menu")

    Note:
        Install the machine with ``app.init_state(T)`` or
        ``app.insert_state(T.MEMBER)`` and read the
        current state with ``world.resource(State[T])`` or a
        ``Res[State[T]]`` system parameter. Calling ``State[T](...)`` raises TypeError.
    """
    def __init__(self, initial_state: StateType) -> None:
        """Create a State resource with an initial state value.

        Usually created via app.init_state() or app.insert_state().
        """

    def get(self) -> StateType:
        """Get the current state value."""

    def state_type(self) -> type[StateType]:
        """The @state enum this machine is for.

        Rust reads the machine's type parameter statically; Python carries it at
        runtime, so generic callers need a way to ask.
        """

    def __eq__(self, other: object) -> bool: ...

class NextState(Generic[StateType], Resource):
    """Pending state transition resource.

    Queue state transitions using set(). The transition will be applied
    by the StateTransition schedule (or manually via Commands).

    Example:
        @system
        def start_game(next_state: ResMut[NextState[GameState]]) -> None:
            next_state.set(GameState.IN_GAME)

    Note:
        Queue a transition with
        ``world.resource(NextState[T]).set(T.MEMBER)`` or declare
        ``next_state: ResMut[NextState[T]]`` in a system.
        Calling ``NextState[T](...)`` raises TypeError.
    """
    def set(self, state: StateType) -> None:
        """Queue a transition, including full same-state lifecycle schedules."""

    def set_if_neq(self, state: StateType) -> None:
        """Queue a transition that skips same-state exit and enter schedules.

        A same-state ``OnTransition`` schedule still runs. An equal transition
        already queued by ``set()`` remains unconditional.
        """

    def state_type(self) -> type[StateType]:
        """The @state enum this machine is for.

        Rust reads the machine's type parameter statically; Python carries it at
        runtime, so generic callers need a way to ask.
        """

    def is_pending(self) -> bool:
        """Check if a transition is pending."""

    def peek_pending(self) -> StateType | None:
        """Get the pending state without removing it."""

    def reset(self) -> None:
        """Clear the pending transition."""

def state(cls: type[StateType]) -> type[StateType]:
    """Decorator to mark an Enum as a valid state type.

    Example:
        @state
        class GameState(Enum):
            MENU = auto()
            IN_GAME = auto()
    """

def in_state(state: StateType) -> SystemCondition[[Res[State[StateType]] | None], bool]:
    """Create a run condition that checks if current state matches target state.

    Args:
        state: The target state to check for

    Returns:
        A ``SystemCondition`` for ``run_if``

    Example:
        app.add_systems(Update, menu_system.run_if(in_state(GameState.MENU)))
    """

# Schedule label types (internal - returned by OnEnter/OnExit/OnTransition functions)
class OnEnterSchedule:
    """Schedule label for systems that run when entering a state.

    This is the internal type returned by OnEnter(). Users typically don't
    need to reference this type directly.
    """
    def __eq__(self, other: object) -> bool: ...

class OnExitSchedule:
    """Schedule label for systems that run when exiting a state.

    This is the internal type returned by OnExit(). Users typically don't
    need to reference this type directly.
    """
    def __eq__(self, other: object) -> bool: ...

class OnTransitionSchedule:
    """Schedule label for systems that run during state transitions.

    This is the internal type returned by OnTransition(). Users typically don't
    need to reference this type directly.
    """
    def __eq__(self, other: object) -> bool: ...

# Schedule labels for state transitions (functions that return schedule labels)
def OnEnter(state: StateType) -> OnEnterSchedule:
    """Create a schedule label for systems that run when entering a state.

    Example:
        app.add_systems(OnEnter(GameState.MENU), setup_menu)
    """

def OnExit(state: StateType) -> OnExitSchedule:
    """Create a schedule label for systems that run when exiting a state.

    Example:
        app.add_systems(OnExit(GameState.MENU), cleanup_menu)
    """

def OnTransition(exited: StateType, entered: StateType) -> OnTransitionSchedule:
    """Create a schedule label for systems that run on a specific state transition.

    Example:
        app.add_systems(OnTransition(GameState.MENU, GameState.IN_GAME), start_game)
    """

# Components for automatic entity lifecycle management
class DespawnOnExit(Component, Generic[StateType]):
    """Component that marks an entity to be despawned when exiting a state.

    Cleanup delivers lifecycle observers and releases entity-scoped observers,
    including those belonging to descendants.

    Example:
        commands.spawn(Transform(), DespawnOnExit(GameState.MENU))
    """
    def __init__(self, state: StateType) -> None: ...
    @property
    def state(self) -> StateType:
        """The state this component is scoped to."""

class DespawnOnEnter(Component, Generic[StateType]):
    """Component that marks an entity to be despawned when entering a state.

    Cleanup delivers lifecycle observers and releases entity-scoped observers,
    including those belonging to descendants.

    Example:
        commands.spawn(Transform(), DespawnOnEnter(GameState.PAUSE_MENU))
    """
    def __init__(self, state: StateType) -> None: ...
    @property
    def state(self) -> StateType:
        """The state this component is scoped to."""
