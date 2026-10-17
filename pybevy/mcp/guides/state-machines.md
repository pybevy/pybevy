# State Machines

Finite state machines using Python Enums for game flow (Menu, Playing, Paused, GameOver). States are app-wide resources with lifecycle schedules (`OnEnter`, `OnExit`) and conditional system execution (`run_if`).

## Imports

State types are in the prelude (`from pybevy.prelude import *`). For explicit imports:

```python
from pybevy.state import state, State, NextState, OnEnter, OnExit, OnTransition, in_state
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

`init_state()` only initializes an absent state machine. Calling it again
preserves the current state and any queued `NextState` transition.
`insert_state()` replaces the current state and clears its pending transition.
Removing `State[T]` suspends that machine's automatic transitions; calling
`init_state(T)` restores its default current state and resets its pending request.
The bare spelling follows the same rule when one machine is registered.

## OnEnter / OnExit / OnTransition Schedules

Systems registered with `OnEnter(state_value)` run once when transitioning **into** that state. `OnExit` runs once when **leaving**.

```python
@system
def setup_menu_ui(commands: Commands) -> None:
    commands.spawn(Node(), Button(), Name("play_button"))

@system
def cleanup_menu(commands: Commands, query: Query[Entity, With[MenuEntity]]) -> None:
    for entity in query:
        commands.entity(entity).despawn()

@system
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

Each automatic pass commits every queued machine before any exit callback,
then runs exit, transition and enter phases across all machines. Callbacks read
all the newly committed states. Requests queued inside a callback wait for the
next pass, including requests for another machine. Exceptions raised by state
comparison or cleanup propagate through `update()` and `run()` without hot
reload. With hot reload, they are reported and the App keeps running.

`OnTransition(exited, entered)` runs only for one exact state change. Both
arguments are positional state members:

```python
@system
def finish_menu_transition() -> None:
    print("menu closed; gameplay is starting")

app.add_systems(
    OnTransition(GamePhase.MENU, GamePhase.PLAYING),
    finish_menu_transition,
)
```

## Automatic Entity Cleanup

Import `DespawnOnExit` and `DespawnOnEnter` from `pybevy.state` and include
the marker in a spawned bundle. `DespawnOnExit(member)` cleans up when leaving
that member; `DespawnOnEnter(member)` cleans up when entering it, including
the initial entry. Descendants are cleaned up too.

These sweeps deliver Python Despawn, Discard and Remove observers while their
old component values are still readable, then release entity-scoped observers
before deleting the entities. Observer query handles expire after each callback.

## Changing State at Runtime

Queue transitions via `ResMut[NextState[StateType]]`. This mirrors Bevy's
`ResMut<NextState<S>>` and selects the exact machine. Transitions are deferred -
they apply between frames, not immediately.

```python
@system
def check_start_game(
    next_state: ResMut[NextState[GamePhase]],
    pressed_query: Query[Entity, tuple[With[PlayButton], Added[Pressed]]],
) -> None:
    for _ in pressed_query:
        next_state.set(GamePhase.PLAYING)
```

`set()` is unconditional, matching Bevy: setting the current member runs that
identity transition's `OnExit`, `OnTransition`, and `OnEnter` schedules. Use
`set_if_different()` for a per-frame driver that should skip same-state exit and
enter work:

```python
@system
def drive_phase(next_state: ResMut[NextState[GamePhase]]) -> None:
    next_state.set_if_different(GamePhase.PLAYING)
```

Bevy still runs `OnTransition(current, current)` for that conditional identity
transition. `NextState` holds one pending request. A later request for a
different target replaces it, even if the earlier `set()` targets the current
state. The exception is `set(X)` followed by `set_if_different(X)`: the unconditional
request remains pending, so same-state exit and enter still run. When requests
come from different systems, explicitly order those systems if the winner
matters; registration order alone is not an execution guarantee.

Bare `ResMut[NextState]` remains compatible when an App has exactly one state
machine. It is ambiguous and raises `TypeError` when multiple machines are registered.
If the App already has multiple machines, `add_systems()` rejects the system.
If another machine is installed later, the same check runs before the system
body on its next attempted execution.

## Reading Current State

```python
@system
def show_hud(current: Res[State[GamePhase]]) -> None:
    phase = current.get()
    # phase is the GamePhase enum value
```

Bare `Res[State]` is a one-machine compatibility shorthand. Prefer the typed
form, especially in reusable systems and Apps with multiple state machines.
Known ambiguity is rejected by `add_systems()`; topology introduced afterward
is checked when the system runs.

## Conditional Systems with run_if

Run systems only when in a specific state:

```python
# This system only runs during PLAYING state
app.add_systems(Update, run_if(move_player, in_state(GamePhase.PLAYING)))
app.add_systems(Update, run_if(enemy_ai, in_state(GamePhase.PLAYING)))
app.add_systems(Update, run_if(pause_menu_input, in_state(GamePhase.PAUSED)))
```

`pybevy.ecs.conditions.state_is_active(GamePhase, GamePhase.PLAYING)` is
equivalent to `in_state(GamePhase.PLAYING)`. Pass the matching `@state` enum
type and member. It reads that machine's `State[GamePhase]` resource and
returns false when the machine is absent. It does not inspect arbitrary
custom-resource fields, which are not safe read-only condition parameters.

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

@system
def enter_menu(commands: Commands) -> None:
    commands.spawn(MenuEntity(), Name("menu_bg"))

@system
def exit_menu(commands: Commands, query: Query[Entity, With[MenuEntity]]) -> None:
    for entity in query:
        commands.entity(entity).despawn()

@system
def enter_playing(commands: Commands) -> None:
    commands.spawn(Transform(), Name("player"))

@system
def start_game(
    next_state: ResMut[NextState[Phase]],
    keys: Res[ButtonInput[KeyCode]],
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

Control-plane `set_resource` requests retain unconditional `set()` semantics.
Repeatedly writing the current member therefore runs the full identity
transition; `set_if_different()` is available through Python systems and World
resource access when conditional lifecycle behavior is required.

Transitions stay deferred: the value is queued and applied by the transition
system on a later frame, so read `State[T]` back rather than assuming the change
landed synchronously.

## State Identity and Reload

State-scoped `DespawnOnExit` and `DespawnOnEnter` markers belong to their enum's
machine. Equal integer values in distinct `IntEnum` types do not share cleanup.
Redefining a state type under the same `module.qualname` keeps its logical
identity across reload; a reused class address alone cannot identify a machine.

Full reload rebuilds declared machines and removes obsolete ones, releasing
retired classes and excluding them from bare-resource ambiguity checks.
Partial reload preserves the machine set and current/pending values, migrating
members of redefined types by name. Use Full reload to remove a machine.

## Known Limitations

- Bare `State` / `NextState` resource parameters require exactly one registered state machine
- State transitions are deferred (apply between frames), not immediate

`in_state(Mode.VALUE)` returns false when `State[Mode]` is absent, including
after resource removal. Use `Optional[Res[T]]` or `Optional[ResMut[T]]`
with `from typing import Optional`, mirroring Bevy's `Option<...>`.
This includes `Optional[Res[Assets[T]]]`: an absent resource supplies `None`,
while an empty collection still supplies a resource. `T | None` also works.
