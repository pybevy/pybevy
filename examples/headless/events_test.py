"""Exercise MessageWriter and MessageReader in the real Bevy loop."""
from __future__ import annotations

from pybevy.decorators import entrypoint, resource
from pybevy.prelude import *


class DamageMessage(Message):
    def __init__(self, amount: float = 0.0):
        self.amount = amount


@resource
class TickCounter(Resource):
    def __init__(self):
        self.tick = 0
        self.total_damage = 0.0


def emit_damage(messages: MessageWriter[DamageMessage]) -> None:
    messages.write(DamageMessage(10.0))


def track_damage(
    messages: MessageReader[DamageMessage],
    counter: ResMut[TickCounter],
    app_exit: MessageWriter[AppExit],
) -> None:
    counter.tick += 1
    for message in messages:
        counter.total_damage += message.amount
    print(
        f"[tick {counter.tick}] total_damage={counter.total_damage:.0f}",
        flush=True,
    )
    if counter.tick >= 5:
        app_exit.write(AppExit.Success())


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(MinimalPlugins)
        .add_message(DamageMessage)
        .add_message(AppExit)
        .insert_resource(TickCounter())
        .add_systems(Update, emit_damage)
        .add_systems(Last, track_damage)
    )


if __name__ == "__main__":
    main().run()
