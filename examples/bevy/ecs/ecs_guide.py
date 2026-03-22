"""Guided introduction to Bevy's Entity Component System (ECS) in PyBevy.

This is a guided introduction to Bevy's "Entity Component System" (ECS).
All Bevy app logic is built using the ECS pattern, so definitely pay attention!

Why ECS?
- Data oriented: Functionality is driven by data
- Clean Architecture: Loose coupling of functionality / prevents deeply nested inheritance
- High Performance: Massively parallel and cache friendly

ECS Definitions:

Component: just a normal Python data type, generally scoped to a single piece of functionality
    Examples: position, velocity, health, color, name

Entity: a collection of components with a unique id
    Examples: Entity1 { Name("Alice"), Position(0, 0) },
              Entity2 { Name("Bill"), Position(10, 5) }

Resource: a shared global piece of data
    Examples: asset storage, messages, system state

System: runs logic on entities, components, and resources
    Examples: move system, damage system

Now that you know a little bit about ECS, let's look at some PyBevy code!
We will now make a simple "game" to illustrate what PyBevy's ECS looks like in practice.

Note: PyBevy doesn't yet support:
- Component field access (use Resources instead - see PlayerData below)
- Custom SystemSet enums, .configure_sets(), .in_set()
- System ordering with .before()/.after() (chain() works for linear ordering)
- Exclusive systems (&mut World parameter)
This example demonstrates the core ECS concepts with these adaptations.
"""

import random
from dataclasses import dataclass
from enum import Enum

from pybevy.app import AppExit, ScheduleRunnerPlugin, chain
from pybevy.prelude import *


@dataclass
class Counter:
    """Simple counter class for Local state."""
    value: int = 0


# COMPONENTS: Pieces of functionality we add to entities. These are just normal Python data types.
#
# Note: PyBevy doesn't yet support component field access, so we use marker components
# and store the actual data in Resources. This is a temporary limitation.


@component
class Player(Component):
    """Marker component for player entities.

    In PyBevy, we can't access fields on components yet, so we use this as a marker
    and store player data (name, score, streak) in the PlayerData resource.
    """


# Enums can be used with components.
# This enum tracks how many consecutive rounds a player has/hasn't scored in.
class StreakType(Enum):
    """Type of streak a player is on."""
    HOT = "hot"      # Scoring consistently
    NONE = "none"    # No streak
    COLD = "cold"    # Not scoring


@dataclass
class PlayerInfo:
    """Data for a single player (stored in PlayerData resource)."""
    entity: Entity
    name: str
    score: int
    streak_type: StreakType
    streak_count: int

    def streak_display(self) -> str:
        """Format streak for display."""
        if self.streak_type == StreakType.HOT:
            return f"{self.streak_count} round hot streak"
        if self.streak_type == StreakType.COLD:
            return f"{self.streak_count} round cold streak"
        return "0 round streak"


# RESOURCES: "Global" state accessible by systems. These are also just normal Python data types!
#


@resource
class GameState(Resource):
    """Resource holding information about the game."""

    def __init__(self):
        self.current_round: int = 0
        self.total_players: int = 0
        self.winning_player: str | None = None


@resource
class GameRules(Resource):
    """Resource providing rules for our game."""

    def __init__(self):
        self.winning_score: int = 4
        self.max_rounds: int = 10
        self.max_players: int = 4


@resource
class PlayerData(Resource):
    """Resource storing all player data.

    Since PyBevy doesn't support component field access yet, we store player
    data in this resource instead. Each player entity has a Player marker component,
    and we look up their data here by entity ID.
    """

    def __init__(self):
        self.players: dict[Entity, PlayerInfo] = {}

    def add_player(self, entity: Entity, name: str) -> None:
        """Add a new player."""
        self.players[entity] = PlayerInfo(
            entity=entity,
            name=name,
            score=0,
            streak_type=StreakType.NONE,
            streak_count=0
        )

    def get_player(self, entity: Entity) -> PlayerInfo | None:
        """Get player data by entity."""
        return self.players.get(entity)


# SYSTEMS: Logic that runs on entities, components, and resources. These generally run once each
# time the app updates.
#


def print_message_system() -> None:
    """Simplest type of system - just prints a message on each run."""
    print("This game is fun!")


def new_round_system(game_rules: Res[GameRules], game_state: ResMut[GameState]) -> None:
    """System that reads and modifies resources.

    This system starts a new round on each update.
    Note: Res[GameRules] is read-only. ResMut[GameState] allows modification.
    """
    game_state.current_round += 1
    print(f"Begin round {game_state.current_round} of {game_rules.max_rounds}")


def score_system(
    query: Query[Entity, With[Player]],
    player_data: ResMut[PlayerData]
) -> None:
    """Update the score for each entity with the Player component.

    This demonstrates:
    - Querying entities with specific components
    - Accessing and modifying resource data
    - Random game logic
    """
    for entity in query:
        player = player_data.get_player(entity)
        if player is None:
            continue

        scored_a_point = random.choice([True, False])

        if scored_a_point:
            player.score += 1

            # Update streak
            if player.streak_type == StreakType.HOT:
                player.streak_count += 1
            else:
                player.streak_type = StreakType.HOT
                player.streak_count = 1

            print(f"{player.name} scored a point! Their score is: {player.score} ({player.streak_display()})")
        else:
            # Update streak
            if player.streak_type == StreakType.COLD:
                player.streak_count += 1
            else:
                player.streak_type = StreakType.COLD
                player.streak_count = 1

            print(f"{player.name} did not score a point! Their score is: {player.score} ({player.streak_display()})")

    # this game isn't very fun is it :)


def score_check_system(
    game_rules: Res[GameRules],
    game_state: ResMut[GameState],
    query: Query[Entity, With[Player]],
    player_data: Res[PlayerData]
) -> None:
    """Check if any player has won.

    This system runs on all entities with the Player component, and also
    accesses the GameRules resource to determine if a player has won.
    """
    for entity in query:
        player = player_data.get_player(entity)
        if player is None:
            continue

        if player.score == game_rules.winning_score:
            game_state.winning_player = player.name


def game_over_system(
    game_rules: Res[GameRules],
    game_state: Res[GameState],
    app_exit_writer: MessageWriter[AppExit]
) -> None:
    """End the game if we meet the right conditions.

    This fires an AppExit message, which tells our App to quit.
    Check out the message.py example to learn more about using messages.
    """
    if game_state.winning_player is not None:
        print(f"{game_state.winning_player} won the game!")
        app_exit_writer.write(AppExit.SUCCESS)
    elif game_state.current_round == game_rules.max_rounds:
        print("Ran out of rounds. Nobody wins!")
        app_exit_writer.write(AppExit.SUCCESS)


def startup_system(
    commands: Commands,
    game_state: ResMut[GameState],
    player_data: ResMut[PlayerData]
) -> None:
    """Startup system that runs exactly once when the app starts up.

    Startup systems are generally used to create the initial state of our game.
    The only thing that distinguishes a startup system from a normal system is
    how it is registered:
        Startup: app.add_systems(Startup, startup_system)
        Normal:  app.add_systems(Update, normal_system)
    """
    # Create our game rules resource
    commands.insert_resource(GameRules())

    # Add some players to our world. Players start with a score of 0 ... we want
    # our game to be fair!
    entity1 = commands.spawn(Player()).id()
    entity2 = commands.spawn(Player()).id()

    player_data.add_player(entity1, "Alice")
    player_data.add_player(entity2, "Bob")

    # Set the total players to 2
    game_state.total_players = 2


def new_player_system(
    commands: Commands,
    game_rules: Res[GameRules],
    game_state: ResMut[GameState],
    player_data: ResMut[PlayerData]
) -> None:
    """System using commands to potentially add a new player on each iteration.

    Commands give us the ability to queue up changes to our World without
    directly accessing it. This is important because normal systems run in
    parallel, and directly accessing the World in parallel is not thread safe.

    Command buffers are applied at the end of each schedule run.
    """
    # Randomly add a new player
    add_new_player = random.choice([True, False])

    if add_new_player and game_state.total_players < game_rules.max_players:
        game_state.total_players += 1
        entity = commands.spawn(Player()).id()
        player_data.add_player(entity, f"Player {game_state.total_players}")
        print(f"Player {game_state.total_players} joined the game!")


def print_at_end_round(counter: Local[Counter]) -> None:
    """System demonstrating Local<T> for per-system state.

    Local<T> refers to a value of type T that is owned by the system.
    This value is automatically initialized using T's default implementation
    upon the system's initialization.

    In this system's Local (counter), T is Counter.
    Therefore, on the first turn, counter has a value of 0.
    """
    counter.value.value += 1
    print(f"In schedule 'Last' for the {counter.value.value}th time")
    # Print an empty line between rounds
    print()


@entrypoint
def main(app: App) -> App:
    """Bevy app's entry point.

    Bevy apps are created using the builder pattern. We use the builder to add
    systems, resources, and plugins to our app.

    Note: PyBevy doesn't yet support system sets (.configure_sets(), .in_set()),
    system ordering (.before(), .after() - only chain() works), or exclusive systems.
    This example demonstrates the core ECS concepts using the available features.
    """
    return (
        app
        # Resources can be added with insert_resource()
        .insert_resource(GameState())
        .insert_resource(PlayerData())
        # Plugins are just a grouped set of app builder calls.
        # The ScheduleRunnerPlugin runs our app's schedule once every 5 seconds.
        .add_plugins(ScheduleRunnerPlugin.run_loop(5))
        # AppExit message must be registered
        .add_message(AppExit)
        # Startup systems run exactly once BEFORE all other systems.
        # These are generally used for app initialization (adding entities and resources).
        .add_systems(Startup, startup_system)
        # Update systems run once every update (generally one "frame" or "tick").
        .add_systems(Update, print_message_system)
        # SYSTEM EXECUTION ORDER
        #
        # By default, all systems in a Schedule run in parallel, except when they
        # require mutable access to the same piece of data.
        #
        # PyBevy supports chain() for linear ordering of systems (max 4 systems).
        # Since we have 5 systems, we split them into two chains.
        .add_systems(
            Update,
            chain(
                new_round_system,
                new_player_system,
                score_system,
                score_check_system,
            )
        )
        .add_systems(Update, game_over_system)
        # The Last schedule runs at the very end of each update.
        .add_systems(Last, print_at_end_round)
    )


if __name__ == "__main__":
    main().run()
