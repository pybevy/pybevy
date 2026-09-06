"""Move the hero — the minimal moving-sprite starter.

Arrow keys / WASD to move. The smallest complete "game": a window, a level
strip, a sprite, and per-frame input-driven movement.

Run from the repo root:  python examples/games/move_the_hero.py
BENCH=1 makes the hero walk a figure by itself (used with PYBEVY_MAX_FRAMES
for headless smoke runs).
"""
import math
import os

from pybevy.prelude import *
from pybevy.window import WindowResolution  # not re-exported by the prelude

BENCH = os.environ.get("BENCH") == "1"  # auto-walk a figure for demos/CI


@component
class Player(Component):
    pass


@resource
class FrameBudget(Resource):
    def __init__(self):
        self.left = int(os.environ.get("PYBEVY_MAX_FRAMES", "0"))


def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    commands.spawn(Camera2d())
    # ground strip (colored rects; PyForge: one add_sprite call per block)
    for gx in range(-400, 401, 40):
        commands.spawn(
            Sprite.from_color(Color.srgb(0.45, 0.72, 0.35), Vec2(40.0, 40.0)),
            Transform.from_xyz(float(gx), -290.0, -1.0),
        )
    hero = Sprite.from_image(asset_server.load_image("games/hero.png"))
    hero.custom_size = (64.0, 64.0)
    commands.spawn(hero, Transform.from_xyz(0.0, -230.0, 0.0), Player())


def move_hero(
    keys: Res[ButtonInput],
    time: Res[Time],
    query: Query[Mut[Transform], With[Player]],
) -> None:
    speed = 300.0
    dt = time.delta_secs()
    if BENCH:
        now = time.elapsed_secs()
        for t in query:
            t.translation.x = math.cos(now * 1.2) * 250.0
            t.translation.y = -230.0 + abs(math.sin(now * 2.4)) * 120.0
        return
    for t in query:
        if keys.pressed(KeyCode.ArrowLeft) or keys.pressed(KeyCode.KeyA):
            t.translation.x -= speed * dt
        if keys.pressed(KeyCode.ArrowRight) or keys.pressed(KeyCode.KeyD):
            t.translation.x += speed * dt
        if keys.pressed(KeyCode.ArrowUp) or keys.pressed(KeyCode.KeyW):
            t.translation.y += speed * dt
        if keys.pressed(KeyCode.ArrowDown) or keys.pressed(KeyCode.KeyS):
            t.translation.y -= speed * dt


def autoclose(budget: ResMut[FrameBudget], exit_writer: MessageWriter[AppExit]) -> None:
    if budget.left <= 0:
        return  # interactive run
    budget.left -= 1
    if budget.left == 0:
        exit_writer.write(AppExit.SUCCESS)


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(ClearColor(Color.srgb(0.53, 0.81, 0.92)))
        .insert_resource(FrameBudget())
        .add_plugins(
            DefaultPlugins().set(
                WindowPlugin(
                    primary_window=Window(
                        title="Move the hero (PyBevy port)",
                        resolution=WindowResolution(800, 600),
                    )
                )
            )
        )
        .add_systems(Startup, setup)
        .add_systems(Update, move_hero)
        .add_systems(Update, autoclose)
    )


if __name__ == "__main__":
    main().run()
