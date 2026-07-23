"""Breakout — paddle, ball, five rows of bricks.

Left/Right (or A/D) move the paddle. Clear the bricks; three lives.
Demonstrates: game-state resource, collision + response, multi-label HUD,
win/lose states.

Run from the repo root:  python examples/games/breakout.py
BENCH=1 = the paddle tracks the ball by itself.
"""
import os

from pybevy.prelude import *
from pybevy.window import WindowResolution  # not re-exported by the prelude

BENCH = os.environ.get("BENCH") == "1"  # paddle tracks the ball (demo/CI autoplay)

RX, TOP, BOTTOM = 390.0, 290.0, -290.0
PADDLE_W, PADDLE_H, BALL, BRICK_W, BRICK_H, GAP = 120.0, 24.0, 22.0, 64.0, 28.0, 6.0
ROW_COLORS = [
    (0.90, 0.30, 0.30),  # red
    (0.95, 0.55, 0.25),  # orange
    (0.95, 0.85, 0.30),  # yellow
    (0.40, 0.80, 0.40),  # green
    (0.95, 0.55, 0.75),  # pink
]


@component
class Paddle(Component):
    pass


@component
class Ball(Component):
    pass


@component
class Brick(Component):
    pass


@component
class ScoreText(Component):
    pass


@component
class LivesText(Component):
    pass


@resource
class GameState(Resource):
    def __init__(self):
        self.vx, self.vy = 220.0, 300.0
        self.score, self.lives, self.over = 0, 3, False
        self.ball_x, self.ball_y = 0.0, -230.0
        self.paddle_x = 0.0
        self.reset_ball = False


@resource
class FrameBudget(Resource):
    def __init__(self):
        self.left = int(os.environ.get("PYBEVY_MAX_FRAMES", "0"))


def rect(color, w, h):
    return Sprite.from_color(Color.srgb(*color), Vec2(w, h))


def setup(commands: Commands) -> None:
    commands.spawn(Camera2d())
    commands.spawn(rect((0.9, 0.9, 0.95), PADDLE_W, PADDLE_H), Transform.from_xyz(0.0, -260.0, 0.0), Paddle())
    commands.spawn(rect((1.0, 1.0, 1.0), BALL, BALL), Transform.from_xyz(0.0, -230.0, 0.0), Ball())
    grid_w = 10 * (BRICK_W + GAP) - GAP
    for r in range(5):
        for c in range(10):
            bx = -grid_w / 2 + BRICK_W / 2 + c * (BRICK_W + GAP)
            by = 250.0 - r * (BRICK_H + GAP)
            commands.spawn(rect(ROW_COLORS[r], BRICK_W, BRICK_H), Transform.from_xyz(bx, by, 0.0), Brick())
    font = TextFont.from_font_size(28.0)
    commands.spawn(Text2d("SCORE 0"), font, Transform.from_xyz(-300.0, 275.0, 0.0), ScoreText())
    commands.spawn(Text2d("LIVES 3"), font, Transform.from_xyz(300.0, 275.0, 0.0), LivesText())


def paddle_move(
    keys: Res[ButtonInput],
    time: Res[Time],
    state: ResMut[GameState],
    query: Query[Mut[Transform], With[Paddle]],
) -> None:
    if state.over:
        return
    speed = 480.0
    dt = time.delta_secs()
    for t in query:
        if BENCH:
            dx = state.ball_x - t.translation.x
            step = min(abs(dx), speed * dt)
            t.translation.x += step if dx > 0 else -step
            t.translation.x = max(-RX + PADDLE_W / 2, min(RX - PADDLE_W / 2, t.translation.x))
        if keys.pressed(KeyCode.ArrowLeft) or keys.pressed(KeyCode.KeyA):
            t.translation.x = max(-RX + PADDLE_W / 2, t.translation.x - speed * dt)
        if keys.pressed(KeyCode.ArrowRight) or keys.pressed(KeyCode.KeyD):
            t.translation.x = min(RX - PADDLE_W / 2, t.translation.x + speed * dt)
        state.paddle_x = t.translation.x


def ball_move(
    time: Res[Time],
    state: ResMut[GameState],
    query: Query[Mut[Transform], With[Ball]],
) -> None:
    if state.over:
        return
    dt = time.delta_secs()
    for t in query:
        if state.reset_ball:
            t.translation.x, t.translation.y = 0.0, -230.0
            state.vx, state.vy = 220.0, 300.0
            state.reset_ball = False
        t.translation.x += state.vx * dt
        t.translation.y += state.vy * dt
        # walls
        if t.translation.x < -RX or t.translation.x > RX:
            state.vx = -state.vx
            t.translation.x = max(-RX, min(RX, t.translation.x))
        if t.translation.y > TOP:
            state.vy = -abs(state.vy)
        # paddle bounce + english (paddle pos shadowed in state — see port notes)
        if (
            state.vy < 0
            and abs(t.translation.x - state.paddle_x) < (BALL + PADDLE_W) / 2
            and abs(t.translation.y - (-260.0)) < (BALL + PADDLE_H) / 2
        ):
            state.vy = abs(state.vy)
            state.vx += (t.translation.x - state.paddle_x) * 3.0
        # miss
        if t.translation.y < BOTTOM:
            state.lives -= 1
            if state.lives <= 0:
                state.over = True
            else:
                state.reset_ball = True
        state.ball_x, state.ball_y = t.translation.x, t.translation.y


def brick_hits(
    state: ResMut[GameState],
    commands: Commands,
    bricks: Query[tuple[Entity, Transform], With[Brick]],
) -> None:
    if state.over:
        return
    remaining = 0
    hit_done = False
    for entity, t in bricks:
        remaining += 1
        if hit_done:
            continue
        if (
            abs(state.ball_x - t.translation.x) < (BALL + BRICK_W) / 2
            and abs(state.ball_y - t.translation.y) < (BALL + BRICK_H) / 2
        ):
            commands.despawn(entity)
            remaining -= 1
            state.vy = -state.vy
            state.score += 10
            hit_done = True  # one brick per frame keeps the bounce clean
    if remaining == 0:
        state.over = True


def score_hud(state: Res[GameState], query: Query[Mut[Text2d], With[ScoreText]]) -> None:
    for text in query:
        if state.over:
            won = state.lives > 0
            text.text = f"{'YOU WIN!' if won else 'GAME OVER'} — SCORE {state.score}"
        else:
            text.text = f"SCORE {state.score}"


def lives_hud(state: Res[GameState], query: Query[Mut[Text2d], With[LivesText]]) -> None:
    for text in query:
        text.text = f"LIVES {state.lives}"


def autoclose(budget: ResMut[FrameBudget], exit_writer: MessageWriter[AppExit]) -> None:
    if budget.left <= 0:
        return
    budget.left -= 1
    if budget.left == 0:
        exit_writer.write(AppExit.SUCCESS)


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(ClearColor(Color.srgb(0.07, 0.08, 0.14)))
        .insert_resource(GameState())
        .insert_resource(FrameBudget())
        .add_plugins(
            DefaultPlugins().set(
                WindowPlugin(
                    primary_window=Window(
                        title="PyBrick (PyBevy port)",
                        resolution=WindowResolution(800, 600),
                    )
                )
            )
        )
        .add_systems(Startup, setup)
        .add_systems(Update, paddle_move)
        .add_systems(Update, ball_move)
        .add_systems(Update, brick_hits)
        .add_systems(Update, score_hud)
        .add_systems(Update, lives_hud)
        .add_systems(Update, autoclose)
    )


if __name__ == "__main__":
    main().run()
