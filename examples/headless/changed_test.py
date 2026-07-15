"""Exercise Changed[T] filters in a short headless app."""
from __future__ import annotations

from pybevy.decorators import component, entrypoint, resource
from pybevy.prelude import *


@component
class Counter(Component):
    def __init__(self, value: int = 0):
        self.value = value


@resource
class TickCounter(Resource):
    def __init__(self):
        self.tick = 0


def setup(commands: Commands) -> None:
    commands.spawn(Counter(0))
    print("[setup] Spawned 1 entity", flush=True)


def increment(query: Query[Mut[Counter]]) -> None:
    """Increment the counter every frame, triggering change detection."""
    for counter in query:
        counter.value += 1


def detect_changed(
    query: Query[Counter, Changed[Counter]],
    tick_counter: ResMut[TickCounter],
    app_exit: MessageWriter[AppExit],
) -> None:
    tick_counter.tick += 1
    changed_count = sum(1 for _counter in query)
    print(
        f"[tick {tick_counter.tick}] Changed matched {changed_count} entities",
        flush=True,
    )
    if tick_counter.tick >= 5:
        app_exit.write(AppExit.SUCCESS)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(MinimalPlugins)
        .add_message(AppExit)
        .insert_resource(TickCounter())
        .add_systems(Startup, setup)
        .add_systems(Update, increment)
        .add_systems(Last, detect_changed)
    )


if __name__ == "__main__":
    main().run()
