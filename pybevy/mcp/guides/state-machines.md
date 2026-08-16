# State Machines

Finite state machines using Python Enums for game flow (Menu, Playing, Paused, GameOver). States are app-wide resources with lifecycle schedules (`OnEnter`, `OnExit`) and conditional system execution (`run_if`).

## Imports

State types are in the prelude (`from pybevy.prelude import *`). For explicit imports:

```python
from pybevy.ecs import state, State, NextState, OnEnter, OnExit, OnTransition, in_state
from enum import Enum, auto
```

## Defining a State

Decorate an `Enum` subclass with `@state`. State identity uses the enum members,
so their underlying values may be integers, strings, or other valid `Enum` values.

```python
from pybevy.prelude import *
from enum import Enum, auto

@state
class GamePhase(Enum):
    MENU = auto()
    PLAYING = auto()
    PAUSED = auto()
    GAME_OVER = auto()
```

## Registering with the App

```python
# Set explicit initial state (preferred)
app.insert_state(GamePhase.MENU)

# Or use default (first variant)
app.init_state(GamePhase)
```

`OnEnter` fires for the initial state on the first frame - no need for a separate Startup system.

## OnEnter / OnExit / OnTransition Schedules

Systems registered with `OnEnter(state_value)` run once when transitioning **into** that state. `OnExit` runs once when **leaving**.

```python
def setup_menu_ui(commands: Commands) -> None:
    commands.spawn(Node(), Button(), Name("play_button"))

def cleanup_menu(commands: Commands, query: Query[Entity, With[MenuEntity]]) -> None:
    for entity in query:
        commands.entity(entity).despawn()

def setup_game_world(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Spawn game entities...
    pass

app.add_systems(OnEnter(GamePhase.MENU), setup_menu_ui)
app.add_systems(OnExit(GamePhase.MENU), cleanup_menu)
app.add_systems(OnEnter(GamePhase.PLAYING), setup_game_world)
```

OnEnter/OnExit systems accept full system parameters (Commands, queries, resources, etc.).

`OnTransition(exited, entered)` runs only for one exact state change. Both
arguments are positional state members:

```python
def finish_menu_transition() -> None:
    print("menu closed; gameplay is starting")

app.add_systems(
    OnTransition(GamePhase.MENU, GamePhase.PLAYING),
    finish_menu_transition,
)
```

## Changing State at Runtime

Queue transitions via `ResMut[NextState[StateType]]`. This mirrors Bevy's
`ResMut<NextState<S>>` and selects the exact machine. Transitions are deferred -
they apply between frames, not immediately.

```python
def check_start_game(
    next_state: ResMut[NextState[GamePhase]],
    interaction_query: Query[Interaction, With[PlayButton]],
) -> None:
    for interaction in interaction_query:
        if interaction.is_pressed():
            next_state.set(GamePhase.PLAYING)
```

Bare `ResMut[NextState]` remains compatible when an App has exactly one state
machine. It is ambiguous and raises `TypeError` when multiple machines are registered.

## Reading Current State

```python
def show_hud(current: Res[State[GamePhase]]) -> None:
    phase = current.get()
    # phase is the GamePhase enum value
```

Bare `Res[State]` is a one-machine compatibility shorthand. Prefer the typed
form, especially in reusable systems and Apps with multiple state machines.

## Conditional Systems with run_if

Run systems only when in a specific state:

```python
# This system only runs during PLAYING state
app.add_systems(Update, run_if(move_player, in_state(GamePhase.PLAYING)))
app.add_systems(Update, run_if(enemy_ai, in_state(GamePhase.PLAYING)))
app.add_systems(Update, run_if(pause_menu_input, in_state(GamePhase.PAUSED)))
```

## Complete Example

```python
from pybevy.prelude import *
from enum import Enum, auto

@state
class Phase(Enum):
    MENU = auto()
    PLAYING = auto()

@component
class MenuEntity(Component):
    pass

def enter_menu(commands: Commands) -> None:
    commands.spawn(MenuEntity(), Name("menu_bg"))

def exit_menu(commands: Commands, query: Query[Entity, With[MenuEntity]]) -> None:
    for entity in query:
        commands.entity(entity).despawn()

def enter_playing(commands: Commands) -> None:
    commands.spawn(Transform(), Name("player"))

def start_game(
    next_state: ResMut[NextState[Phase]],
    keys: Res[ButtonInput],
) -> None:
    if keys.just_pressed(KeyCode.Space):
        next_state.set(Phase.PLAYING)

@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_state(Phase.MENU)
        .add_systems(OnEnter(Phase.MENU), enter_menu)
        .add_systems(OnExit(Phase.MENU), exit_menu)
        .add_systems(OnEnter(Phase.PLAYING), enter_playing)
        .add_systems(Update, run_if(start_game, in_state(Phase.MENU)))
    )

if __name__ == "__main__":
    main().run()
```

## Inspecting and Driving States as an Agent

`State[T]` and `NextState[T]` report the active member through `get_resource`,
and accept a transition through `set_resource`, using the same `variant`
convention as enum components:

```text
get_resource {"resource_type": "State[game.Phase]"}
# {"name": "State[game.Phase]", "present": true, "fields": {"variant": "MENU"}}

# queue a transition
set_resource {"resource_type": "NextState[game.Phase]", "value": {"variant": "PLAYING"}}
```

The bare `NextState` name works when exactly one machine is registered, matching
the system-parameter rule above. An unknown member is rejected rather than
silently queuing nothing.

Transitions stay deferred: the value is queued and applied by the transition
system on a later frame, so read `State[T]` back rather than assuming the change
landed synchronously.

## Known Limitations

- Bare `State` / `NextState` resource parameters require exactly one registered state machine
- State transitions are deferred (apply between frames), not immediate
