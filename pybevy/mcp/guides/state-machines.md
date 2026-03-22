# State Machines

Finite state machines using Python Enums for game flow (Menu, Playing, Paused, GameOver). States are app-wide resources with lifecycle schedules (`OnEnter`, `OnExit`) and conditional system execution (`run_if`).

## Imports

State types are in the prelude (`from pybevy.prelude import *`). For explicit imports:

```python
from pybevy.ecs import state, State, NextState, OnEnter, OnExit, OnTransition, in_state
from enum import Enum, auto
```

## Defining a State

Decorate an `Enum` subclass with `@state`. Values must be integers.

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

`OnEnter` fires for the initial state on the first frame — no need for a separate Startup system.

## OnEnter / OnExit Schedules

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

## Changing State at Runtime

Queue transitions via `ResMut[NextState]`. Transitions are deferred — they apply between frames, not immediately.

```python
def check_start_game(
    next_state: ResMut[NextState],
    interaction_query: Query[Interaction, With[PlayButton]],
) -> None:
    for interaction in interaction_query:
        if interaction.pressed():
            next_state.set(GamePhase.PLAYING)
```

**IMPORTANT**: Use `ResMut[NextState]` — **not** `ResMut[NextState[GamePhase]]`. The generic subscript is not supported at runtime.

## Reading Current State

```python
def show_hud(current: Res[State]) -> None:
    phase = current.get()
    # phase is the GamePhase enum value
```

Use `Res[State]` — **not** `Res[State[GamePhase]]`.

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

def start_game(next_state: ResMut[NextState], keys: Res[ButtonInput]) -> None:
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

## Known Limitations

- `State[T]` and `NextState[T]` generic subscripts don't work at runtime — use bare `State` / `NextState`
- Hot reload crashes with `OnEnter`/`OnExit` schedules — use `run_scene` to restart
- `DespawnOnExit[T]()` generic subscript doesn't work — manually despawn in `OnExit` systems
- State transitions are deferred (apply between frames), not immediate
