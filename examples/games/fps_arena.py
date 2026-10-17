"""FPS arena — first-person shooter with billboard enemies.

Up/W forward, Down/S back, Left/Right turn, click to shoot.
Demonstrates: 3D meshes + materials, camera-facing textured billboards,
first-person camera from a pose resource, Res[MouseInput] hitscan,
UI-node HUD over 3D, one-shot audio.

Run from the repo root:  python examples/games/fps_arena.py
BENCH=1 = auto-turn + auto-fire.
"""
import math
import os

import numpy as np

from pybevy.input import MouseInput
from pybevy.prelude import *
from pybevy.window import WindowResolution

BENCH = os.environ.get("BENCH") == "1"
FRAMES = int(os.environ.get("PYBEVY_MAX_FRAMES", "0"))
RNG = np.random.default_rng(3)
FRAME_T: list[float] = []

# static arena solids as XZ half-extent boxes: (cx, cz, hx, hz) — walls + pillars
SOLIDS = [(0.0, -15.0, 15.0, 0.3), (0.0, 15.0, 15.0, 0.3), (-15.0, 0.0, 0.3, 15.0), (15.0, 0.0, 0.3, 15.0)]
PILLARS = [(-6.0, -6.0), (6.0, 6.0), (-6.0, 6.0), (6.0, -6.0)]
SOLIDS += [(px, pz, 1.0, 1.0) for px, pz in PILLARS]


@component
class Wizard(Component):
    pass


@component
class HudText(Component):
    pass


@resource
class Pose(Resource):
    def __init__(self):
        self.px, self.pz, self.yaw = 0.0, 11.0, math.pi
        self.frame = 0


@resource
class GameState(Resource):
    def __init__(self):
        self.kills, self.total = 0, 9


def collide(px: float, pz: float, r: float = 0.5) -> tuple[float, float]:
    # straight port of PyForge's collide_3d (axis-of-least-penetration vs XZ boxes)
    for cx, cz, hx, hz in SOLIDS:
        ox = hx + r - abs(px - cx)
        oz = hz + r - abs(pz - cz)
        if ox > 0.0 and oz > 0.0:
            if ox < oz:
                px = px - ox if px < cx else px + ox
            else:
                pz = pz - oz if pz < cz else pz + oz
    return px, pz


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
    asset_server: Res[AssetServer],
) -> None:
    cube = meshes.add(Cuboid.from_length(1.0))

    def box(x, y, z, sx, sy, sz, color):
        commands.spawn(
            Mesh3d(cube),
            MeshMaterial3d(materials.add(StandardMaterial.from_color(Color.srgb(*color)))),
            Transform.from_xyz(x, y, z).with_scale(Vec3(sx, sy, sz)),
        )

    box(0, -0.25, 0, 30, 0.5, 30, (0.20, 0.19, 0.24))  # floor
    for x, z, dx, dz in [(0, -15, 30, 0.6), (0, 15, 30, 0.6), (-15, 0, 0.6, 30), (15, 0, 0.6, 30)]:
        box(x, 2, z, dx, 4, dz, (0.40, 0.38, 0.50))  # walls
    for px, pz in PILLARS:
        box(px, 1.5, pz, 2, 3, 2, (0.50, 0.45, 0.56))  # pillars

    # wizard billboards: unlit textured quads, rotated to face the camera each frame
    quad = meshes.add(Rectangle(2.0, 2.0))
    wiz_mat = materials.add(
        StandardMaterial(
            base_color_texture=asset_server.load_image("games/wizard.png"),
            unlit=True,
            alpha_mode=AlphaMode.Blend(),
        )
    )
    for _ in range(9):
        ex, ez = float(RNG.uniform(-12, 12)), float(RNG.uniform(-12, 12))
        if abs(ex) < 4 and abs(ez) < 4:
            ez += 7.0
        commands.spawn(Mesh3d(quad), MeshMaterial3d(wiz_mat), Transform.from_xyz(ex, 1.0, ez), Wizard())

    commands.spawn(
        DirectionalLight(illuminance=9000.0),
        Transform.from_xyz(4.0, 9.0, 6.0).looking_at(Vec3.ZERO, Vec3.Y),
    )
    commands.spawn(Camera3d(), Transform.from_xyz(0.0, 1.4, 11.0))

    # UI HUD over the 3D view: kill counter + crosshair (two absolute nodes)
    hud = Node()
    hud.position_type = PositionType.Absolute
    hud.top = Val.px(20.0)
    hud.left = Val.px(24.0)
    commands.spawn(hud, Text("KILLS 0 / 9"), HudText())
    for w, h, left, top in [(3.0, 22.0, 398.5, 289.0), (22.0, 3.0, 389.0, 298.5)]:
        n = Node()
        n.position_type = PositionType.Absolute
        n.width = Val.px(w)
        n.height = Val.px(h)
        n.left = Val.px(left)
        n.top = Val.px(top)
        commands.spawn(n, BackgroundColor(Color.srgb(1.0, 0.3, 0.3)))
    print(f"pydoom port: BENCH={BENCH} FRAMES={FRAMES}")


def move_player(keys: Res[ButtonInput], time: Res[Time], pose: ResMut[Pose]) -> None:
    dt, speed, turn = time.delta_secs(), 5.0, 2.0
    pose.frame += 1
    if BENCH:
        pose.yaw += 0.6 * dt  # slow sweep so the aim cone crosses the wizards
    if keys.pressed(KeyCode.ArrowLeft) or keys.pressed(KeyCode.KeyA):
        pose.yaw += turn * dt
    if keys.pressed(KeyCode.ArrowRight) or keys.pressed(KeyCode.KeyD):
        pose.yaw -= turn * dt
    dx, dz = math.sin(pose.yaw), math.cos(pose.yaw)
    nx, nz = pose.px, pose.pz
    if keys.pressed(KeyCode.ArrowUp) or keys.pressed(KeyCode.KeyW):
        nx += dx * speed * dt
        nz += dz * speed * dt
    if keys.pressed(KeyCode.ArrowDown) or keys.pressed(KeyCode.KeyS):
        nx -= dx * speed * dt
        nz -= dz * speed * dt
    pose.px, pose.pz = collide(nx, nz)


def sync_camera(pose: Res[Pose], q: Query[Mut[Transform], With[Camera3d]]) -> None:
    for t in q:
        t.translation.x, t.translation.y, t.translation.z = pose.px, 1.4, pose.pz
        # default camera forward is -Z; ours is (sin yaw, 0, cos yaw) => yaw + pi
        t.rotation = Quat.from_rotation_y(pose.yaw + math.pi)


def wizards_creep(time: Res[Time], pose: Res[Pose], q: Query[Mut[Transform], With[Wizard]]) -> None:
    dt = time.delta_secs()
    face = Quat.from_rotation_y(pose.yaw + math.pi)  # quad +Z ends up facing the camera
    for t in q:
        ex, ez = pose.px - t.translation.x, pose.pz - t.translation.z
        d = math.hypot(ex, ez) or 1.0
        t.translation.x += ex / d * 1.1 * dt
        t.translation.z += ez / d * 1.1 * dt
        t.rotation = face


def shoot(
    mouse: Res[MouseInput],
    pose: Res[Pose],
    state: ResMut[GameState],
    commands: Commands,
    wizards: Query[tuple[Entity, Transform], With[Wizard]],
    asset_server: Res[AssetServer],
) -> None:
    # NB: MouseButton.Left is a FACTORY METHOD, not an enum value — it must be
    # called. `e.button == MouseButton.Left` compares against the method object
    # and is silently always False (the original bug here). PyBevy's own
    # MouseInput docstring shows the uncalled form, which raises a TypeError.
    clicked = mouse.just_pressed(MouseButton.Left())
    if BENCH and pose.frame % 30 == 0:
        clicked = True
    if not clicked:
        return
    commands.spawn(AudioPlayer(asset_server.load_audio("games/pop.ogg")), PlaybackSettings.DESPAWN)
    dx, dz = math.sin(pose.yaw), math.cos(pose.yaw)
    target, best = None, 1e9
    for entity, t in wizards:
        ex, ez = t.translation.x - pose.px, t.translation.z - pose.pz
        d = math.hypot(ex, ez)
        if d > 0.2 and (ex * dx + ez * dz) / d > 0.96 and d < best:  # ~16° cone
            target, best = entity, d
    if target is not None:
        commands.despawn(target)
        state.kills += 1


def hud(state: Res[GameState], q: Query[Mut[Text], With[HudText]]) -> None:
    for text in q:
        text.content = f"KILLS {state.kills} / {state.total}"


def report(state: Res[GameState], exit_writer: MessageWriter[AppExit]) -> None:
    import time as pytime

    FRAME_T.append(pytime.perf_counter())
    if FRAMES == 0 or len(FRAME_T) < FRAMES:
        return
    spans = np.diff(FRAME_T[60:])
    fps = 1.0 / float(np.median(spans)) if len(spans) else 0.0
    ok = fps >= 59.0 and (state.kills >= 3 if BENCH else True)
    print(
        f"RESULT kills={state.kills}/{state.total} fps={fps:.0f} "
        f"-> {'PASS' if ok else 'FAIL'} (gate: 60fps, hitscan+despawn+audio+HUD exercised)"
    )
    exit_writer.write(AppExit.SUCCESS)


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(ClearColor(Color.srgb(0.06, 0.06, 0.10)))
        .insert_resource(Pose())
        .insert_resource(GameState())
        .add_plugins(
            DefaultPlugins().set(
                WindowPlugin(
                    primary_window=Window(
                        title="PyDoom (PyBevy port)",
                        resolution=WindowResolution(800, 600),
                    )
                )
            )
        )
        .add_systems(Startup, setup)
        .add_systems(Update, move_player)
        .add_systems(Update, sync_camera)
        .add_systems(Update, wizards_creep)
        .add_systems(Update, shoot)
        .add_systems(Update, hud)
        .add_systems(Update, report)
    )


if __name__ == "__main__":
    main().run()
