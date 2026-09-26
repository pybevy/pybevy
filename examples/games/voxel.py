"""Voxel builder — break and place blocks, Minecraft-style.

Up/W forward, Down/S back, Left/Right turn. Click breaks the block under
the crosshair; Space places one. Demonstrates: runtime 3D entity churn,
a pure-Python ray-march over the voxel grid, shared mesh/material handles.

Run from the repo root:  python examples/games/voxel.py
BENCH=1 = orbit the terrain, breaking and placing automatically.
"""
import math
import os
from dataclasses import dataclass

import numpy as np

from pybevy.input import MouseInput
from pybevy.prelude import *
from pybevy.window import WindowResolution

BENCH = os.environ.get("BENCH") == "1"
FRAMES = int(os.environ.get("PYBEVY_MAX_FRAMES", "0"))
EYE_Y, PITCH = 3.0, -0.7
PLACE_COLOR = (0.85, 0.35, 0.35)
FRAME_T: list[float] = []
HANDLES: dict = {}  # mesh + material handles, filled in setup


@component
@dataclass
class Block(Component):
    gx: float = 0.0
    gy: float = 0.0
    gz: float = 0.0


@resource
class Pose(Resource):
    def __init__(self):
        self.px, self.pz, self.yaw = 0.0, 9.0, math.pi
        self.frame = 0


@resource
class World(Resource):
    def __init__(self):
        self.cells: set = set()  # occupied (gx,gy,gz)
        self.broken, self.placed = 0, 0


def block_color(gy, top):
    if top and gy >= 0:
        return (0.42, 0.72, 0.34)  # grass
    return (0.55, 0.42, 0.30) if gy >= 0 else (0.42, 0.40, 0.44)  # dirt / stone


def forward(yaw):
    fx, fy, fz = math.sin(yaw), PITCH, math.cos(yaw)
    fl = math.sqrt(fx * fx + fy * fy + fz * fz)
    return fx / fl, fy / fl, fz / fl


def ray_pick(cells, ex, ey, ez, fx, fy, fz):
    """March the aim ray; return (hit_cell, empty_cell_before_it)."""
    prev = None
    for s in range(1, 140):
        t = s * 0.12
        cell = (round(ex + fx * t), round(ey + fy * t), round(ez + fz * t))
        if cell in cells:
            return cell, prev
        prev = cell
    return None, None


def spawn_block(commands, cell, mat):
    gx, gy, gz = cell
    commands.spawn(
        Mesh3d(HANDLES["mesh"]),
        MeshMaterial3d(mat),
        Transform.from_xyz(float(gx), float(gy), float(gz)),
        Block(gx=float(gx), gy=float(gy), gz=float(gz)),
    )


def setup(
    commands: Commands,
    world: ResMut[World],
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    HANDLES["mesh"] = meshes.add(Cuboid.from_length(1.0))
    for key, color in [("grass", block_color(0, True)), ("dirt", block_color(0, False)),
                       ("stone", block_color(-1, False)), ("place", PLACE_COLOR)]:
        HANDLES[key] = materials.add(StandardMaterial.from_color(Color.srgb(*color)))

    # gentle voxel terrain (same formula as the original)
    for gx in range(-6, 6):
        for gz in range(-6, 6):
            h = int(round(1.4 * (math.sin(gx * 0.5) + math.cos(gz * 0.5))))
            h = max(0, min(3, h))
            for gy in range(-1, h + 1):
                key = "grass" if gy == h and gy >= 0 else ("dirt" if gy >= 0 else "stone")
                spawn_block(commands, (gx, gy, gz), HANDLES[key])
                world.cells.add((gx, gy, gz))

    commands.spawn(
        DirectionalLight(illuminance=9000.0),
        Transform.from_xyz(4.0, 9.0, 6.0).looking_at(Vec3.ZERO, Vec3.Y),
    )
    commands.spawn(Camera3d(), Transform.from_xyz(0.0, EYE_Y, 9.0))

    # crosshair + instructions (UI layer, same pattern as the pydoom port)
    for w, h, left, top in [(3.0, 20.0, 398.5, 290.0), (20.0, 3.0, 390.0, 298.5)]:
        n = Node()
        n.position_type = PositionType.Absolute
        n.width = Val.px(w)
        n.height = Val.px(h)
        n.left = Val.px(left)
        n.top = Val.px(top)
        commands.spawn(n, BackgroundColor(Color.srgb(1.0, 1.0, 1.0)))
    info = Node()
    info.position_type = PositionType.Absolute
    info.top = Val.px(16.0)
    info.left = Val.px(20.0)
    commands.spawn(info, Text("click: break    space: place"))
    print(f"pycraft port: BENCH={BENCH} FRAMES={FRAMES} blocks={len(world.cells)}")


def move_player(keys: Res[ButtonInput], time: Res[Time], pose: ResMut[Pose]) -> None:
    dt, speed, turn = time.delta_secs(), 5.0, 2.0
    pose.frame += 1
    if BENCH:
        # orbit the terrain, always facing the center (a blind 360° spin spends
        # most of the take staring at empty sky)
        a = time.elapsed_secs() * 0.35
        pose.px = math.sin(a) * 9.0
        pose.pz = math.cos(a) * 9.0
        pose.yaw = math.atan2(-pose.px, -pose.pz)
    if keys.pressed(KeyCode.ArrowLeft) or keys.pressed(KeyCode.KeyA):
        pose.yaw += turn * dt
    if keys.pressed(KeyCode.ArrowRight) or keys.pressed(KeyCode.KeyD):
        pose.yaw -= turn * dt
    dx, dz = math.sin(pose.yaw), math.cos(pose.yaw)
    if keys.pressed(KeyCode.ArrowUp) or keys.pressed(KeyCode.KeyW):
        pose.px += dx * speed * dt
        pose.pz += dz * speed * dt
    if keys.pressed(KeyCode.ArrowDown) or keys.pressed(KeyCode.KeyS):
        pose.px -= dx * speed * dt
        pose.pz -= dz * speed * dt


def sync_camera(pose: Res[Pose], q: Query[Mut[Transform], With[Camera3d]]) -> None:
    fx, fy, fz = forward(pose.yaw)
    look = Transform.from_xyz(pose.px, EYE_Y, pose.pz).looking_at(
        Vec3(pose.px + fx, EYE_Y + fy, pose.pz + fz), Vec3.Y
    )
    for t in q:
        t.translation.x, t.translation.y, t.translation.z = pose.px, EYE_Y, pose.pz
        t.rotation = look.rotation


def edit_blocks(
    mouse: Res[MouseInput],
    keys: Res[ButtonInput],
    pose: Res[Pose],
    world: ResMut[World],
    commands: Commands,
    blocks: Query[tuple[Entity, Block], With[Block]],
) -> None:
    breaking = mouse.just_pressed(MouseButton.Left())
    placing = keys.just_pressed(KeyCode.Space)
    if BENCH and pose.frame % 40 == 0:
        breaking = True
    if BENCH and pose.frame % 40 == 20:
        placing = True
    if not (breaking or placing):
        return

    fx, fy, fz = forward(pose.yaw)
    hit, place_cell = ray_pick(world.cells, pose.px, EYE_Y, pose.pz, fx, fy, fz)

    if breaking and hit:
        for entity, b in blocks:
            if (round(b.gx), round(b.gy), round(b.gz)) == hit:
                commands.despawn(entity)
                world.cells.discard(hit)
                world.broken += 1
                break
    if placing and place_cell and place_cell not in world.cells:
        spawn_block(commands, place_cell, HANDLES["place"])
        world.cells.add(place_cell)
        world.placed += 1


def report(world: Res[World], exit_writer: MessageWriter[AppExit]) -> None:
    import time as pytime

    FRAME_T.append(pytime.perf_counter())
    if FRAMES == 0 or len(FRAME_T) < FRAMES:
        return
    spans = np.diff(FRAME_T[60:])
    fps = 1.0 / float(np.median(spans)) if len(spans) else 0.0
    ok = fps >= 59.0 and (world.broken >= 2 and world.placed >= 1 if BENCH else True)
    print(
        f"RESULT blocks={len(world.cells)} broken={world.broken} placed={world.placed} "
        f"fps={fps:.0f} -> {'PASS' if ok else 'FAIL'} (gate: 60fps, break+place churn works)"
    )
    exit_writer.write(AppExit.SUCCESS)


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(ClearColor(Color.srgb(0.53, 0.74, 0.96)))
        .insert_resource(Pose())
        .insert_resource(World())
        .add_plugins(
            DefaultPlugins().set(
                WindowPlugin(
                    primary_window=Window(
                        title="PyCraft (PyBevy port) — click break, Space place",
                        resolution=WindowResolution(800, 600),
                    )
                )
            )
        )
        .add_systems(Startup, setup)
        .add_systems(Update, move_player)
        .add_systems(Update, sync_camera)
        .add_systems(Update, edit_blocks)
        .add_systems(Update, report)
    )


if __name__ == "__main__":
    main().run()
