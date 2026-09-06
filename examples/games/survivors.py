"""Survivors — survive the horde (a Vampire-Survivors-style stress demo).

WASD/arrows move; your wand auto-fires at the nearest foe; touching foes
drains HP. Demonstrates: heavy spawn/despawn churn, View column DSL for
batched enemy movement, a per-frame NumPy position snapshot for collision,
and a live HUD. Holds thousands of enemies at full frame rate.

Run from the repo root:  python examples/games/survivors.py
BENCH=1 floods to MAX foes (default 1000) with damage off — a stress test.
"""
import os
import statistics
import time as pytime
from dataclasses import dataclass

import numpy as np

from pybevy.prelude import *
from pybevy.window import WindowResolution

BENCH = os.environ.get("BENCH") == "1"
MAX_ENEMIES = int(os.environ.get("MAX", "1000" if BENCH else "200"))
FRAMES = int(os.environ.get("PYBEVY_MAX_FRAMES", "0"))
W2, H2 = 450.0, 325.0

RNG = np.random.default_rng(11)
FRAME_T: list[float] = []


@component
class Player(Component):
    pass


@component
class Enemy(Component):
    pass


@component
@dataclass
class Bolt(Component):
    vx: float = 0.0
    vy: float = 0.0
    life: float = 1.1


@component
@dataclass
class Spark(Component):
    vx: float = 0.0
    vy: float = 0.0
    life: float = 0.4


@component
class HpText(Component):
    pass


@component
class KillText(Component):
    pass


@component
class FpsText(Component):
    pass


@resource
class GameState(Resource):
    def __init__(self):
        self.px, self.py = 0.0, 0.0
        self.hp, self.kills, self.over = 100.0, 0, False
        self.spawn_t, self.fire_t = 0.0, 0.0
        self.alive = 0  # live enemy count (spawns - kills)
        self.sparks_spawned = 0


def rect(color, size):
    return Sprite.from_color(Color.srgb(*color), Vec2(size, size))


def setup(commands: Commands) -> None:
    commands.spawn(Camera2d())
    commands.spawn(rect((0.4, 0.8, 1.0), 42.0), Transform.from_xyz(0.0, 0.0, 5.0), Player())
    font = TextFont.from_font_size(24.0)
    commands.spawn(Text2d("HP 100"), font, Transform.from_xyz(-420.0, 305.0, 10.0), HpText())
    commands.spawn(Text2d("KILLS 0"), font, Transform.from_xyz(-120.0, 305.0, 10.0), KillText())
    commands.spawn(Text2d(""), font, Transform.from_xyz(230.0, 305.0, 10.0), FpsText())
    print(f"pysurvivors port: BENCH={BENCH} MAX={MAX_ENEMIES} FRAMES={FRAMES}")


def player_move(
    keys: Res[ButtonInput],
    time: Res[Time],
    state: ResMut[GameState],
    q: Query[Mut[Transform], With[Player]],
) -> None:
    if state.over:
        return
    dt, speed = time.delta_secs(), 250.0
    for t in q:
        if keys.pressed(KeyCode.ArrowLeft) or keys.pressed(KeyCode.KeyA):
            t.translation.x -= speed * dt
        if keys.pressed(KeyCode.ArrowRight) or keys.pressed(KeyCode.KeyD):
            t.translation.x += speed * dt
        if keys.pressed(KeyCode.ArrowUp) or keys.pressed(KeyCode.KeyW):
            t.translation.y += speed * dt
        if keys.pressed(KeyCode.ArrowDown) or keys.pressed(KeyCode.KeyS):
            t.translation.y -= speed * dt
        t.translation.x = max(-W2 + 10, min(W2 - 10, t.translation.x))
        t.translation.y = max(-H2 + 15, min(H2 - 15, t.translation.y))
        state.px, state.py = t.translation.x, t.translation.y


def spawner(time: Res[Time], state: ResMut[GameState], commands: Commands) -> None:
    if state.over or state.alive >= MAX_ENEMIES:
        return
    n = 0
    if BENCH:
        n = min(25, MAX_ENEMIES - state.alive)  # flood to cap fast
    else:
        state.spawn_t += time.delta_secs()
        if state.spawn_t > 0.05:
            state.spawn_t, n = 0.0, 1
    for _ in range(n):
        side = int(RNG.integers(0, 4))
        if side == 0:
            ex, ey = float(RNG.uniform(-460, 460)), 340.0
        elif side == 1:
            ex, ey = float(RNG.uniform(-460, 460)), -340.0
        elif side == 2:
            ex, ey = -480.0, float(RNG.uniform(-330, 330))
        else:
            ex, ey = 480.0, float(RNG.uniform(-330, 330))
        commands.spawn(rect((0.9, 0.3, 0.3), 26.0), Transform.from_xyz(ex, ey, 1.0), Enemy())
        state.alive += 1


def enemy_move(time: Res[Time], state: Res[GameState], view: View[Mut[Transform], With[Enemy]]) -> None:
    if state.over or state.alive == 0:
        return
    dt = time.delta_secs()
    try:
        pos = view.column_mut(Transform)
        dx = state.px - pos.translation.x
        dy = state.py - pos.translation.y
        d = (dx * dx + dy * dy).sqrt().max(1e-4)
        pos.translation.x += dx / d * (68.0 * dt)
        pos.translation.y += dy / d * (68.0 * dt)
    except Exception:
        return  # spawn commands not yet applied (first frame of the horde)


# NOTE: these two were one system, but With-only filters (With[Bolt] +
# With[Spark]) don't PROVE disjointness, so two Mut[Transform] queries are
# rejected at registration — correct Bevy semantics. Adding mutual Without
# filters would merge them; split kept for clarity.
def bolt_move(time: Res[Time], bolts: Query[tuple[Mut[Transform], Bolt], With[Bolt]]) -> None:
    dt = time.delta_secs()
    for t, b in bolts:
        t.translation.x += b.vx * dt
        t.translation.y += b.vy * dt


def spark_move(
    time: Res[Time],
    commands: Commands,
    sparks: Query[tuple[Entity, Mut[Transform], Mut[Spark]], With[Spark]],
) -> None:
    dt = time.delta_secs()
    for entity, t, s in sparks:
        s.life -= dt
        if s.life <= 0:
            commands.despawn(entity)
            continue
        t.translation.x += s.vx * dt
        t.translation.y += s.vy * dt
        k = max(0.05, s.life / 0.4)  # shrink instead of alpha fade
        t.scale.x = k
        t.scale.y = k


def combat(
    time: Res[Time],
    state: ResMut[GameState],
    commands: Commands,
    enemies: Query[tuple[Entity, Transform], With[Enemy]],
    bolts: Query[tuple[Entity, Transform, Mut[Bolt]], With[Bolt]],
) -> None:
    if state.over:
        return
    dt = time.delta_secs()

    # one snapshot per frame, reused for touch + targeting + hits
    ents, exs, eys = [], [], []
    for entity, t in enemies:
        ents.append(entity)
        exs.append(t.translation.x)
        eys.append(t.translation.y)
    ex = np.asarray(exs, dtype=np.float32)
    ey = np.asarray(eys, dtype=np.float32)

    # touch damage
    if len(ents) and not BENCH:
        touching = int(np.sum((np.abs(ex - state.px) < 22) & (np.abs(ey - state.py) < 22)))
        if touching:
            state.hp -= touching * 12.0 * dt
            if state.hp <= 0:
                state.over = True

    # auto-fire at the nearest enemy
    state.fire_t += dt
    if state.fire_t > 0.16 and len(ents):
        state.fire_t = 0.0
        d2 = (ex - state.px) ** 2 + (ey - state.py) ** 2
        i = int(np.argmin(d2))
        d = float(np.sqrt(d2[i])) or 1.0
        vx, vy = (float(ex[i]) - state.px) / d * 460.0, (float(ey[i]) - state.py) / d * 460.0
        commands.spawn(
            rect((1.0, 0.95, 0.4), 9.0),
            Transform.from_xyz(state.px, state.py, 4.0),
            Bolt(vx=vx, vy=vy),
        )

    # bolt hits (vectorized per bolt) + lifetime
    killed: set[int] = set()
    for entity, t, b in bolts:
        b.life -= dt
        hit = False
        if len(ents):
            mask = (np.abs(ex - t.translation.x) < 16) & (np.abs(ey - t.translation.y) < 16)
            for i in np.flatnonzero(mask):
                if int(i) in killed:
                    continue
                killed.add(int(i))
                commands.despawn(ents[int(i)])
                state.kills += 1
                state.alive -= 1
                for _ in range(4):
                    a = float(RNG.uniform(0, 6.28))
                    v = float(RNG.uniform(70, 170))
                    commands.spawn(
                        rect((1.0, 0.6, 0.2), 13.0),
                        Transform.from_xyz(float(ex[i]), float(ey[i]), 3.0),
                        Spark(vx=np.cos(a) * v, vy=np.sin(a) * v),
                    )
                    state.sparks_spawned += 1
                hit = True
                break
        if hit or b.life <= 0:
            commands.despawn(entity)


def hud_hp(state: Res[GameState], q: Query[Mut[Text2d], With[HpText]]) -> None:
    for text in q:
        text.text = "YOU DIED" if state.over else f"HP {max(0, int(state.hp))}"


def hud_kills(state: Res[GameState], q: Query[Mut[Text2d], With[KillText]]) -> None:
    for text in q:
        text.text = f"KILLS {state.kills}"


def hud_fps(state: Res[GameState], q: Query[Mut[Text2d], With[FpsText]]) -> None:
    FRAME_T.append(pytime.perf_counter())
    if len(FRAME_T) % 30 or len(FRAME_T) < 31:
        return
    fps = 29.0 / (FRAME_T[-1] - FRAME_T[-30])
    for text in q:
        text.text = f"{state.alive} foes  {fps:.0f} fps"


def report(state: Res[GameState], exit_writer: MessageWriter[AppExit]) -> None:
    if FRAMES == 0 or len(FRAME_T) < FRAMES:
        return
    spans = np.diff(FRAME_T[60:])
    fps = 1.0 / float(np.median(spans)) if len(spans) else 0.0
    cap_ok = state.alive >= MAX_ENEMIES * 0.9 if BENCH else True  # faithful mode spawns 20/s
    ok = fps >= 59.0 and cap_ok and state.kills > 0
    print(
        f"RESULT foes={state.alive} kills={state.kills} fps={fps:.0f} "
        f"-> {'PASS' if ok else 'FAIL'}"
    )
    exit_writer.write(AppExit.SUCCESS)


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(ClearColor(Color.srgb(0.08, 0.07, 0.11)))
        .insert_resource(GameState())
        .add_plugins(
            DefaultPlugins().set(
                WindowPlugin(
                    primary_window=Window(
                        title="PySurvivors (PyBevy port)",
                        resolution=WindowResolution(900, 650),
                    )
                )
            )
        )
        .add_systems(Startup, setup)
        .add_systems(Update, player_move)
        .add_systems(Update, spawner)
        .add_systems(Update, enemy_move)
        .add_systems(Update, bolt_move)
        .add_systems(Update, spark_move)
        .add_systems(Update, combat)
        .add_systems(Update, hud_hp)
        .add_systems(Update, hud_kills)
        .add_systems(Update, hud_fps)
        .add_systems(Update, report)
    )


if __name__ == "__main__":
    main().run()
