"""This example demonstrates using system parameters like Query and Resource.

Note: Unlike Rust Bevy, PyBevy doesn't support creating custom SystemParam types.
Instead, system functions can directly use Query, Commands, Resources, etc. as parameters.
"""

from pybevy.prelude import *


@component
class Player(Component):
    pass


@resource
class PlayerCount(Resource):
    def __init__(self):
        self.count: int = 0


def spawn(commands: Commands) -> None:
    """Spawn some players to count."""
    commands.spawn(Player())
    commands.spawn(Player())
    commands.spawn(Player())


def count_players(query: Query[Player], player_count: ResMut[PlayerCount]) -> None:
    """Count players using Query and Resource parameters directly."""
    player_count.count = len(list(query))
    print(f"{player_count.count} players in the game")


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(PlayerCount())
        .add_systems(Startup, spawn)
        .add_systems(Update, count_players)
    )


if __name__ == "__main__":
    main().run()
