"""Tetris — a fixed grid of 200 sprites recolored live every frame.

Left/Right move, Up rotate, Down soft-drop, Space hard-drop.
Demonstrates: Mut[Sprite] live recoloring (zero churn), edge-triggered
input, pure-Python game logic driving the ECS.

Run from the repo root:  python examples/games/tetris.py
BENCH=1 = deterministic auto-play.
"""
import os
from dataclasses import dataclass

import numpy as np

from pybevy.prelude import *
from pybevy.window import WindowResolution

BENCH = os.environ.get("BENCH") == "1"
FRAMES = int(os.environ.get("PYBEVY_MAX_FRAMES", "0"))
COLS, ROWS, CELL = 10, 20, 28
BG = (0.13, 0.15, 0.20)
FRAME_T: list[float] = []
RNG = np.random.default_rng(5)

SHAPES = {
    "I": [(-1, 0), (0, 0), (1, 0), (2, 0)],
    "O": [(0, 0), (1, 0), (0, 1), (1, 1)],
    "T": [(-1, 0), (0, 0), (1, 0), (0, 1)],
    "S": [(0, 0), (1, 0), (-1, 1), (0, 1)],
    "Z": [(-1, 0), (0, 0), (0, 1), (1, 1)],
    "J": [(-1, 0), (0, 0), (1, 0), (1, 1)],
    "L": [(-1, 0), (0, 0), (1, 0), (-1, 1)],
}
COLORS = {
    "I": (0.3, 0.8, 0.9), "O": (0.9, 0.85, 0.3), "T": (0.7, 0.4, 0.9),
    "S": (0.4, 0.8, 0.4), "Z": (0.9, 0.4, 0.4), "J": (0.4, 0.5, 0.9), "L": (0.9, 0.6, 0.3),
}
BAG = list(SHAPES)


@component
@dataclass
class Cell(Component):
    col: float = 0.0
    row: float = 0.0


@component
class LinesText(Component):
    pass


@resource
class Tetris(Resource):
    def __init__(self):
        self.board = [[None] * COLS for _ in range(ROWS)]
        self.kind, self.offs, self.color = "T", list(SHAPES["T"]), COLORS["T"]
        self.px, self.py = 4, 0
        self.fall, self.interval = 0.0, 0.5
        self.lines, self.locked, self.over = 0, 0, False
        self.bag_i = 0
        self.frame = 0

    def cells_at(self, px, py, offs):
        return [(px + dx, py + dy) for dx, dy in offs]

    def valid(self, offs, px, py):
        for cx, cy in self.cells_at(px, py, offs):
            if cx < 0 or cx >= COLS or cy >= ROWS:
                return False
            if cy >= 0 and self.board[cy][cx] is not None:
                return False
        return True

    def spawn(self):
        kind = BAG[self.bag_i % len(BAG)]
        self.bag_i += 1
        self.kind, self.offs, self.color = kind, list(SHAPES[kind]), COLORS[kind]
        self.px, self.py = 4, 0
        if not self.valid(self.offs, 4, 0):
            self.over = True

    def lock(self):
        for cx, cy in self.cells_at(self.px, self.py, self.offs):
            if cy >= 0:
                self.board[cy][cx] = self.color
        cleared = [r for r in range(ROWS) if all(self.board[r])]
        for r in cleared:
            del self.board[r]
            self.board.insert(0, [None] * COLS)
        self.lines += len(cleared)
        self.locked += 1
        self.spawn()


def setup(commands: Commands, state: ResMut[Tetris]) -> None:
    commands.spawn(Camera2d())
    well_w, well_h = COLS * CELL, ROWS * CELL
    frame = Sprite.from_color(Color.srgb(0.20, 0.22, 0.30), Vec2(well_w + 18.0, well_h + 18.0))
    commands.spawn(frame, Transform.from_xyz(0.0, 0.0, -2.0))

    ox = -(COLS * CELL) / 2 + CELL / 2
    oy = (ROWS * CELL) / 2 - CELL / 2
    for r in range(ROWS):
        for c in range(COLS):
            commands.spawn(
                Sprite.from_color(Color.srgb(*BG), Vec2(CELL - 2.0, CELL - 2.0)),
                Transform.from_xyz(ox + c * CELL, oy - r * CELL, 0.0),
                Cell(col=float(c), row=float(r)),
            )
    commands.spawn(
        Text2d("LINES 0"), TextFont.from_font_size(26.0),
        Transform.from_xyz(0.0, 300.0, 1.0), LinesText(),
    )
    state.spawn()
    print(f"pytris port: BENCH={BENCH} FRAMES={FRAMES}")


def game_logic(keys: Res[ButtonInput], time: Res[Time], state: ResMut[Tetris]) -> None:
    if state.over:
        return
    state.frame += 1
    left = keys.just_pressed(KeyCode.ArrowLeft) or keys.just_pressed(KeyCode.KeyA)
    right = keys.just_pressed(KeyCode.ArrowRight) or keys.just_pressed(KeyCode.KeyD)
    rotate = keys.just_pressed(KeyCode.ArrowUp) or keys.just_pressed(KeyCode.KeyW)
    hard = keys.just_pressed(KeyCode.Space)
    soft = keys.pressed(KeyCode.ArrowDown) or keys.pressed(KeyCode.KeyS)
    if BENCH:  # deterministic auto-play: wiggle, rotate, then slam
        # time-based pacing (one action beat @ 60Hz) — frame-modulo breaks on
        # unthrottled displays (observed 734fps)
        beat = int(time.elapsed_secs() * 60) % 45
        prev = getattr(state, "last_beat", -1)
        state.last_beat = beat
        fresh = beat != prev
        lane = state.locked % 3  # spread pieces across the well, not one tower
        left = fresh and beat in (5, 15) and lane == 0
        right = fresh and beat in (5, 15) and lane == 2
        rotate, hard = fresh and beat == 25, fresh and beat == 35

    if left and state.valid(state.offs, state.px - 1, state.py):
        state.px -= 1
    if right and state.valid(state.offs, state.px + 1, state.py):
        state.px += 1
    if rotate and state.kind != "O":
        r = [(-dy, dx) for dx, dy in state.offs]
        if state.valid(r, state.px, state.py):
            state.offs = r
    if hard:
        while state.valid(state.offs, state.px, state.py + 1):
            state.py += 1
        state.lock()
        return

    state.fall += time.delta_secs()
    step = 0.05 if soft else state.interval
    if state.fall >= step:
        state.fall = 0.0
        if state.valid(state.offs, state.px, state.py + 1):
            state.py += 1
        else:
            state.lock()


def paint(state: Res[Tetris], cells: Query[tuple[Mut[Sprite], Cell], With[Cell]]) -> None:
    # 200 live recolors per frame — the fixed-grid pattern, via Mut[Sprite]
    active = {(cx, cy): state.color
              for cx, cy in state.cells_at(state.px, state.py, state.offs)}
    for sp, cell in cells:
        c, r = int(cell.col), int(cell.row)
        color = active.get((c, r)) or state.board[r][c] or BG
        sp.color = Color.srgb(*color)


def hud(state: Res[Tetris], q: Query[Mut[Text2d], With[LinesText]]) -> None:
    for text in q:
        text.text = "GAME OVER" if state.over else f"LINES {state.lines}"


def report(state: Res[Tetris], exit_writer: MessageWriter[AppExit]) -> None:
    import time as pytime

    FRAME_T.append(pytime.perf_counter())
    if FRAMES == 0 or len(FRAME_T) != FRAMES:  # != so the exit lag doesn't re-print
        return
    spans = np.diff(FRAME_T[60:])
    fps = 1.0 / float(np.median(spans)) if len(spans) else 0.0
    ok = fps >= 59.0 and (state.locked >= 5 if BENCH else True)
    print(
        f"RESULT locked={state.locked} lines={state.lines} over={state.over} "
        f"fps={fps:.0f} -> {'PASS' if ok else 'FAIL'} (gate: 60fps, grid recolor + logic works)"
    )
    exit_writer.write(AppExit.SUCCESS)


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(ClearColor(Color.srgb(0.05, 0.06, 0.10)))
        .insert_resource(Tetris())
        .add_plugins(
            DefaultPlugins().set(
                WindowPlugin(
                    primary_window=Window(
                        title="PyTris (PyBevy port)",
                        resolution=WindowResolution(560, 640),
                    )
                )
            )
        )
        .add_systems(Startup, setup)
        .add_systems(Update, game_logic)
        .add_systems(Update, paint)
        .add_systems(Update, hud)
        .add_systems(Update, report)
    )


if __name__ == "__main__":
    main().run()
