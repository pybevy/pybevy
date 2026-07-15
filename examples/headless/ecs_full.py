"""Exercise custom components, filters, messages, and bridge components."""
from __future__ import annotations

from pybevy.decorators import component, entrypoint, resource
from pybevy.prelude import *


@component
class Health(Component):
    def __init__(self, value: float = 100.0):
        self.value = value


@component
class Poison(Component):
    def __init__(self, dps: float = 10.0):
        self.dps = dps


@component
class Shield(Component):
    pass


class DamageMessage(Message):
    def __init__(self, target: str = "", amount: float = 0.0):
        self.target = target
        self.amount = amount


@resource
class Stats(Resource):
    def __init__(self):
        self.tick = 0
        self.total_damage = 0.0


def setup(commands: Commands) -> None:
    commands.spawn(Health(100.0), Poison(10.0), Name("Warrior"))
    commands.spawn(Health(200.0), Poison(20.0), Shield(), Name("Tank"))
    commands.spawn(Health(50.0), Name("Healer"))
    print("[setup] Spawned 3 entities", flush=True)


def apply_poison(
    query: Query[tuple[Mut[Health], Poison, Name], Without[Shield]],
    messages: MessageWriter[DamageMessage],
) -> None:
    for health, poison, name in query:
        health.value = max(health.value - poison.dps, 0.0)
        messages.write(DamageMessage(str(name), poison.dps))


def apply_shielded_poison(
    query: Query[tuple[Mut[Health], Poison, Name], With[Shield]],
    messages: MessageWriter[DamageMessage],
) -> None:
    for health, poison, name in query:
        damage = poison.dps * 0.5
        health.value = max(health.value - damage, 0.0)
        messages.write(DamageMessage(str(name), damage))


def track_and_report(
    messages: MessageReader[DamageMessage],
    query: Query[tuple[Health, Name]],
    stats: ResMut[Stats],
    app_exit: MessageWriter[AppExit],
) -> None:
    for message in messages:
        stats.total_damage += message.amount
    stats.tick += 1
    health = ", ".join(f"{name}:{value.value:.0f}" for value, name in query)
    print(
        f"[tick {stats.tick}] {health} | damage={stats.total_damage:.0f}",
        flush=True,
    )
    if stats.tick >= 5:
        app_exit.write(AppExit.SUCCESS)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(MinimalPlugins)
        .add_message(DamageMessage)
        .add_message(AppExit)
        .insert_resource(Stats())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (apply_poison, apply_shielded_poison),
        )
        .add_systems(Last, track_and_report)
    )


if __name__ == "__main__":
    main().run()
