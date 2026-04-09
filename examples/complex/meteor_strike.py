"""Meteor Strike — GPU physics demolition with Newton/Warp.

A heavy sphere slams into a wall of 6,000 cubes at high speed, sending debris
flying with real rigid-body physics. Uses NVIDIA Newton (Warp) directly for
GPU-accelerated collision detection and XPBD solving, with Numba for fast
ECS writeback.

Requirements:
- NVIDIA GPU with CUDA
- `pip install newton warp numba`

Features:
- 6,000 dynamic cubes + 1 meteor + ground plane — all GPU-simulated
- Newton XPBD solver with restitution and friction
- View API + Numba JIT for batch Transform writeback from GPU state
- Fiery meteor with ember trail, volumetric fog, and dramatic bloom
- Warm-to-cool color gradient wall with metallic sheen

Controls:
    P = launch / relaunch meteor
"""

import math
import random
import sys
import threading
import time
from dataclasses import dataclass

_missing = []
for _mod in ("newton", "warp", "numba"):
    try:
        __import__(_mod)
    except ImportError:
        _missing.append(_mod)
if _missing:
    print(f"ERROR: Missing required packages: {', '.join(_missing)}")
    print("Install with: pip install newton warp numba")
    sys.exit(1)

import newton  # type: ignore[import-not-found]
import numba
import numpy as np

from pybevy.light import FogVolume, NotShadowCaster, VolumetricFog, VolumetricLight
from pybevy.prelude import *

CUBE_SIZE = 0.25
WALL_COLS = 40   # width (Z)
WALL_ROWS = 30   # height (Y)
WALL_DEPTH = 5   # thickness (X)
CUBE_GAP = 0.01
CUBE_STRIDE = CUBE_SIZE + CUBE_GAP

METEOR_RADIUS = 1.5
METEOR_SPEED = 60.0
METEOR_MASS = 40000.0

EMBER_COUNT = 80

DEVICE = "cuda:0"
DT = 1.0 / 60.0
SUBSTEPS = 6
SOLVER_ITERATIONS = 6

# Slow-motion
SLOWMO_TRIGGER_X = -6.0   # meteor X where slow-mo begins
SLOWMO_IMPACT_X = -1.5    # approximate wall front face X
SLOWMO_MIN_SCALE = 0.05   # slowest time scale (1/20th speed)
SLOWMO_RAMPUP_DURATION = 3.0  # seconds (real time) to ramp back to full speed after impact

# States
STATE_BUILDING = "building"
STATE_READY = "ready"
STATE_RUNNING = "running"


@component
@dataclass
class PhysicsId(Component):
    """Maps an ECS entity to a Newton body index."""
    body_idx: float = 0.0  # float for Numba View compatibility


@component
@dataclass
class Ember(Component):
    """Fire trail particle that follows the meteor."""
    phase: float = 0.0
    radius: float = 1.0
    speed: float = 1.0
    offset_y: float = 0.0


@component
class MeteorTag(Component):
    """Marker for the meteor entity."""


@component
class StatusText(Component):
    """Marker for the on-screen status text."""


@resource
class NewtonSim(Resource):
    """Holds the Newton simulation state."""

    def __init__(self):
        self.model = None
        self.state_0 = None
        self.state_1 = None
        self.control = None
        self.solver = None
        self.poses = np.zeros((0, 7), dtype=np.float32)
        self.state = STATE_BUILDING
        self._build_thread = None
        self._build_result = None
        self._body_data = None
        self.meteor_body_idx = -1
        self.time_scale = 1.0
        self.slowmo_phase = "none"  # none -> approaching -> impact -> rampup
        self.rampup_elapsed = 0.0

    def step(self) -> None:
        if self.model is None or self.state_0 is None or self.state_1 is None or self.solver is None:
            return
        scaled_dt = DT * self.time_scale
        sub_dt = scaled_dt / SUBSTEPS
        for _ in range(SUBSTEPS):
            contacts = self.model.collide(self.state_0)
            self.solver.step(self.state_0, self.state_1, self.control, contacts, sub_dt)
            self.state_0, self.state_1 = self.state_1, self.state_0

    def start_rebuild(self):
        """Kick off a background rebuild of the Newton model."""
        if self._body_data is None or self._build_thread is not None:
            return
        self.state = STATE_BUILDING

        def _build():
            try:
                self._build_result = _build_newton_model(self._body_data)
            except Exception as e:
                print(f"[Newton] BUILD ERROR: {e}")
                self._build_result = None

        self._build_thread = threading.Thread(target=_build, daemon=True)
        self._build_thread.start()

    def apply_build(self):
        """Apply completed build results. Returns True if applied."""
        if self._build_thread is None or self._build_thread.is_alive():
            return False
        self._build_thread = None
        r = self._build_result
        self._build_result = None
        if r is None:
            print("[Newton] Build failed!")
            return False
        self.model = r["model"]
        self.state_0 = r["state_0"]
        self.state_1 = r["state_1"]
        self.control = r["control"]
        self.solver = r["solver"]
        self.poses = np.zeros((self.model.body_count, 7), dtype=np.float32)
        self.read_poses()
        self.state = STATE_READY
        print(f"[Newton] Ready — {self.model.body_count} bodies on GPU")
        return True

    def read_poses(self):
        if self.state_0 is not None and self.model is not None:
            self.poses[:] = self.state_0.body_q.numpy()

    def meteor_pos(self):
        """Get current meteor position as (x, y, z) or None."""
        if self.meteor_body_idx < 0 or len(self.poses) == 0:
            return None
        p = self.poses[self.meteor_body_idx]
        return (p[0], p[1], p[2])


def _build_newton_model(body_data):
    """Build Newton model on background thread. Returns dict with results."""
    t0 = time.perf_counter()
    mb = newton.ModelBuilder()
    mb.up_axis = newton.Axis.Y
    mb.gravity = -9.81

    for body_type, shape, params, pose, velocity in body_data:
        body_idx = mb.add_body()
        mb.body_q[body_idx] = pose

        if velocity is not None:
            mb.body_qd[body_idx] = velocity

        cfg = mb.ShapeConfig()
        cfg.mu = params.get("friction", 0.5)
        cfg.restitution = params.get("restitution", 0.1)
        cfg.density = params.get("density", 1000.0)

        if shape == "box":
            mb.add_shape_box(body=body_idx, hx=params["hx"], hy=params["hy"], hz=params["hz"], cfg=cfg)
        elif shape == "sphere":
            mb.add_shape_sphere(body=body_idx, radius=params["radius"], cfg=cfg)
        elif shape == "plane":
            mb.add_shape_plane(plane=(0.0, 1.0, 0.0, 0.0), body=body_idx, cfg=cfg)

        if body_type == "dynamic":
            mb.add_joint_free(child=body_idx)

    t1 = time.perf_counter()
    model = mb.finalize(device=DEVICE)
    t2 = time.perf_counter()
    state_0 = model.state()
    state_1 = model.state()
    control = model.control()
    solver = newton.solvers.SolverXPBD(model, iterations=SOLVER_ITERATIONS, enable_restitution=True)
    t3 = time.perf_counter()
    # Warm up Warp JIT (first collide triggers kernel compilation)
    _ = model.collide(state_0)
    t4 = time.perf_counter()
    print(f"[Newton] build={1e3*(t1-t0):.0f}ms  finalize={1e3*(t2-t1):.0f}ms  solver={1e3*(t3-t2):.0f}ms  warmup={1e3*(t4-t3):.0f}ms  bodies={model.body_count}")
    return {"model": model, "state_0": state_0, "state_1": state_1, "control": control, "solver": solver}


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
    sim: ResMut[NewtonSim],
):
    random.seed(42)
    t_start = time.perf_counter()

    commands.insert_resource(ClearColor(Color.srgb(0.01, 0.01, 0.03)))
    commands.insert_resource(GlobalAmbientLight(brightness=300.0, color=Color.srgb(0.5, 0.5, 0.65)))

    body_data: list[tuple[str, str, dict[str, float], list[float], list[float] | None]] = []
    body_idx = 0

    # Ground
    commands.spawn(
        Mesh3d(meshes.add(Cuboid(200.0, 0.3, 200.0))),
        MeshMaterial3d(materials.add(StandardMaterial(
            base_color=Color.srgb(0.08, 0.08, 0.12), metallic=0.7, perceptual_roughness=0.25,
        ))),
        Transform.from_xyz(0.0, -0.15, 0.0),
    )
    body_data.append(("static", "box", {"hx": 100.0, "hy": 0.15, "hz": 100.0,
                                        "friction": 0.8, "restitution": 0.02, "density": 0.0},
                       [0.0, -0.15, 0.0, 0.0, 0.0, 0.0, 1.0], None))
    body_idx += 1  # ground = body 0

    # Cube wall
    cube_mesh = meshes.add(Cuboid.from_length(CUBE_SIZE))
    half = CUBE_SIZE / 2.0
    cube_density = 1000.0

    row_materials = []
    impact_row = int(WALL_ROWS * 0.4)
    for row in range(WALL_ROWS):
        t = row / max(WALL_ROWS - 1, 1)
        r = 0.70 - 0.40 * t + random.uniform(-0.04, 0.04)
        g = 0.20 + 0.20 * t + random.uniform(-0.03, 0.03)
        b = 0.10 + 0.55 * t + random.uniform(-0.04, 0.04)
        dist_from_impact = abs(row - impact_row) / WALL_ROWS
        glow = max(0.0, 1.0 - dist_from_impact * 5.0)
        emissive = LinearRgba.rgb(2.0 * glow * r * 3, 0.6 * glow * g * 2, 0.1 * glow) if glow > 0.01 else LinearRgba.rgb(0.0, 0.0, 0.0)
        mat = materials.add(StandardMaterial(
            base_color=Color.srgb(max(r, 0.05), max(g, 0.05), max(b, 0.05)),
            emissive=emissive,
            metallic=0.2 + 0.4 * t,
            perceptual_roughness=0.55 - 0.2 * t,
        ))
        row_materials.append(mat)

    wall_start_z = -(WALL_COLS - 1) * CUBE_STRIDE / 2
    wall_start_y = CUBE_SIZE / 2

    for row in range(WALL_ROWS):
        y = wall_start_y + row * CUBE_STRIDE
        mat = row_materials[row]
        for col in range(WALL_COLS):
            z = wall_start_z + col * CUBE_STRIDE
            for d in range(WALL_DEPTH):
                x = (d - WALL_DEPTH / 2) * CUBE_STRIDE
                commands.spawn(
                    PhysicsId(body_idx=float(body_idx)),
                    Mesh3d(cube_mesh),
                    MeshMaterial3d(mat),
                    Transform.from_xyz(x, y, z),
                )
                body_data.append(("dynamic", "box",
                                   {"hx": half, "hy": half, "hz": half,
                                    "friction": 0.5, "restitution": 0.1, "density": cube_density},
                                   [x, y, z, 0.0, 0.0, 0.0, 1.0], None))
                body_idx += 1

    t_wall = time.perf_counter()
    total_cubes = WALL_COLS * WALL_ROWS * WALL_DEPTH
    print(f"Wall: {total_cubes} cubes — spawn={1e3*(t_wall-t_start):.0f}ms")

    # Meteor (fiery glowing sphere)
    meteor_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(1.0, 0.6, 0.15),
        emissive=LinearRgba.rgb(40.0, 16.0, 3.0),
        unlit=True,
    ))
    impact_y = WALL_ROWS * CUBE_STRIDE * 0.4
    commands.spawn(
        MeteorTag(),
        PhysicsId(body_idx=float(body_idx)),
        Mesh3d(meshes.add(Sphere(METEOR_RADIUS))),
        MeshMaterial3d(meteor_mat),
        Transform.from_xyz(-20.0, impact_y, 0.0),
    )
    sim.meteor_body_idx = body_idx
    meteor_volume = (4.0 / 3.0) * 3.14159 * METEOR_RADIUS ** 3
    body_data.append(("dynamic", "sphere",
                       {"radius": METEOR_RADIUS, "friction": 0.3, "restitution": 0.3,
                        "density": METEOR_MASS / meteor_volume},
                       [-20.0, impact_y, 0.0, 0.0, 0.0, 0.0, 1.0],
                       [METEOR_SPEED, 0.0, 0.0, 0.0, 0.0, 0.0]))
    body_idx += 1

    # Ember trail (small additive-blend spheres orbiting the meteor)
    ember_meshes = [
        meshes.add(Sphere(0.12)),
        meshes.add(Sphere(0.20)),
        meshes.add(Sphere(0.30)),
    ]
    ember_materials = [
        materials.add(StandardMaterial(
            base_color=Color.srgba(1.0, 0.7, 0.2, 0.6),
            emissive=LinearRgba.rgb(20.0, 8.0, 1.5),
            alpha_mode=AlphaMode.Add(),
            unlit=True,
        )),
        materials.add(StandardMaterial(
            base_color=Color.srgba(1.0, 0.4, 0.05, 0.5),
            emissive=LinearRgba.rgb(28.0, 7.0, 0.5),
            alpha_mode=AlphaMode.Add(),
            unlit=True,
        )),
        materials.add(StandardMaterial(
            base_color=Color.srgba(1.0, 0.9, 0.5, 0.4),
            emissive=LinearRgba.rgb(15.0, 10.0, 3.0),
            alpha_mode=AlphaMode.Add(),
            unlit=True,
        )),
    ]
    for i in range(EMBER_COUNT):
        phase = i * math.tau / EMBER_COUNT + random.uniform(-0.8, 0.8)
        radius = METEOR_RADIUS * (0.6 + random.uniform(0.0, 2.0))
        speed = 1.5 + random.uniform(0.0, 4.0)
        offset_y = random.uniform(-1.2, 1.2)
        commands.spawn(
            Ember(phase=phase, radius=radius, speed=speed, offset_y=offset_y),
            Mesh3d(ember_meshes[i % 3]),
            MeshMaterial3d(ember_materials[i % 3]),
            NotShadowCaster(),
            Transform.from_xyz(-20.0, impact_y, 0.0),
        )

    # Fog volumes
    commands.spawn(
        FogVolume(
            density_factor=0.2,
            fog_color=Color.srgb(1.0, 0.5, 0.2),
            absorption=0.05,
            scattering=0.5,
            scattering_asymmetry=0.6,
        ),
        Transform.from_xyz(-2.0, 3.0, 0.0).with_scale(Vec3(10.0, 6.0, 14.0)),
    )
    commands.spawn(
        FogVolume(
            density_factor=0.12,
            fog_color=Color.srgb(1.0, 0.3, 0.05),
            absorption=0.02,
            scattering=0.3,
            scattering_asymmetry=0.8,
        ),
        Transform.from_xyz(-20.0, impact_y, 0.0).with_scale(Vec3(8.0, 4.0, 6.0)),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-22.0, 10.0, 22.0).looking_at(Vec3(-2.0, 3.5, 0.0), Vec3.Y),
        Bloom(intensity=0.45, low_frequency_boost=0.8),
        VolumetricFog(
            ambient_color=Color.srgb(0.03, 0.03, 0.05),
            ambient_intensity=0.05,
            step_count=48,
            jitter=1.0,
        ),
        DistanceFog(
            color=Color.srgb(0.02, 0.02, 0.04),
            falloff=FogFalloff.Exponential(0.008),
            directional_light_color=Color.srgb(1.0, 0.7, 0.3),
            directional_light_exponent=40.0,
        ),
    )

    # Lights
    # Key light (warm sun with volumetric god rays)
    commands.spawn(
        DirectionalLight(illuminance=10000.0, color=Color.srgb(1.0, 0.92, 0.8), shadows_enabled=True),
        VolumetricLight(),
        Transform.from_xyz(10.0, 25.0, 15.0).looking_at(Vec3.ZERO, Vec3.Y),
    )
    # Fill light (cool, no shadows)
    commands.spawn(
        DirectionalLight(illuminance=4000.0, color=Color.srgb(0.6, 0.65, 0.9), shadows_enabled=False),
        Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.4, -1.5, 0.0)),
    )
    # Warm point light near meteor start (fire glow on surroundings)
    commands.spawn(
        PointLight(intensity=1200000.0, color=Color.srgb(1.0, 0.5, 0.15), range=50.0, shadows_enabled=False),
        Transform.from_xyz(-20.0, 8.0, 0.0),
    )
    # Cool accent from the other side
    commands.spawn(
        PointLight(intensity=300000.0, color=Color.srgb(0.3, 0.4, 1.0), range=30.0, shadows_enabled=False),
        Transform.from_xyz(15.0, 10.0, 0.0),
    )

    # Status text (UI overlay)
    root_node = Node()
    root_node.position_type = 1  # Absolute
    root_node.width = Val.px(800.0)
    root_node.top = Val.px(40.0)
    root_node.left = Val.px(400.0)
    root_node.justify_content = JustifyContent.Center
    commands.spawn(
        StatusText(),
        root_node,
        Text("Building physics..."),
        TextFont(font_size=36.0),
        TextColor(Color.srgb(1.0, 0.9, 0.5)),
        TextLayout(justify=Justify.Center),
    )

    # Start background Newton build
    sim._body_data = body_data
    sim.start_rebuild()


@numba.njit(parallel=True, fastmath=True)
def _scatter_poses(translation, rotation, body_ids, poses):
    """Write Newton body poses into ECS Transform arrays."""
    for i in numba.prange(len(translation.x)):
        idx = int(body_ids[i])
        translation.x[i] = poses[idx, 0]
        translation.y[i] = poses[idx, 1]
        translation.z[i] = poses[idx, 2]
        rotation.x[i] = poses[idx, 3]
        rotation.y[i] = poses[idx, 4]
        rotation.z[i] = poses[idx, 5]
        rotation.w[i] = poses[idx, 6]


def poll_build(
    sim: ResMut[NewtonSim],
    status_query: Query[Mut[Text], With[StatusText]],
):
    """Check if background Newton build is done."""
    if sim.state != STATE_BUILDING:
        return
    if sim._build_thread is None or sim._build_thread.is_alive():
        return
    if sim.apply_build():
        for text in status_query:
            text.content = "Press P to launch!"
    else:
        for text in status_query:
            text.content = "Build failed!"


def handle_input(
    keys: Res[ButtonInput],
    sim: ResMut[NewtonSim],
    status_query: Query[Mut[Text], With[StatusText]],
):
    """P = launch when ready, or trigger rebuild when running."""
    if not keys.just_pressed(KeyCode.KeyP):
        return

    if sim.state == STATE_READY:
        sim.state = STATE_RUNNING
        sim.time_scale = 1.0
        sim.slowmo_phase = "none"
        sim.rampup_elapsed = 0.0
        for text in status_query:
            text.content = ""
        print("Meteor launched!")

    elif sim.state == STATE_RUNNING:
        sim.start_rebuild()
        for text in status_query:
            text.content = "Rebuilding..."
        print("Rebuilding physics...")


def step_physics(sim: ResMut[NewtonSim]):
    """Step Newton simulation each frame."""
    if sim.state != STATE_RUNNING:
        return
    sim.step()
    sim.read_poses()


def sync_transforms(
    sim: Res[NewtonSim],
    view: View[tuple[Mut[Transform], PhysicsId]],
):
    """Scatter GPU poses back to ECS Transforms via Numba."""
    if sim.model is None or len(sim.poses) == 0:
        return
    poses = sim.poses
    for batch in view.iter_batches():
        tf = batch.column_mut(Transform)
        ids = batch.column(PhysicsId)
        _scatter_poses(
            tf.translation, tf.rotation,
            ids.body_idx, poses,
        )


def animate_embers(
    sim: Res[NewtonSim],
    time: Res[Time],
    query: Query[tuple[Mut[Transform], Ember]],
):
    """Orbit ember particles around the meteor's current position."""
    pos = sim.meteor_pos()
    if pos is None:
        return
    mx, my, mz = pos
    t = time.elapsed_secs()
    for transform, ember in query:
        angle = t * ember.speed + ember.phase
        # Elongated trail behind the meteor (stretches in -X)
        trail_x = -ember.radius * 1.0 * (1.0 + math.sin(angle * 0.7) * 0.4)
        transform.translation.x = mx + trail_x
        transform.translation.y = my + ember.offset_y + math.sin(angle * 1.3) * ember.radius * 0.35
        transform.translation.z = mz + math.sin(angle) * ember.radius * 0.4 + math.cos(angle * 0.8) * ember.radius * 0.25


def slowmo_controller(sim: ResMut[NewtonSim]):
    """Matrix-style slow-motion as the meteor approaches the wall."""
    if sim.state != STATE_RUNNING:
        return
    pos = sim.meteor_pos()
    if pos is None:
        return
    mx = pos[0]

    if sim.slowmo_phase == "none":
        if mx > SLOWMO_TRIGGER_X:
            sim.slowmo_phase = "approaching"
            print("[SlowMo] Approaching...")

    if sim.slowmo_phase == "approaching":
        # Ramp down: linear from 1.0 at trigger to MIN at impact
        progress = (mx - SLOWMO_TRIGGER_X) / (SLOWMO_IMPACT_X - SLOWMO_TRIGGER_X)
        progress = max(0.0, min(1.0, progress))
        sim.time_scale = 1.0 - progress * (1.0 - SLOWMO_MIN_SCALE)
        if mx > SLOWMO_IMPACT_X:
            sim.slowmo_phase = "impact"
            sim.time_scale = SLOWMO_MIN_SCALE
            sim.rampup_elapsed = 0.0
            print("[SlowMo] IMPACT!")

    elif sim.slowmo_phase == "impact":
        # Hold at minimum for a beat, then start ramp-up
        sim.rampup_elapsed += DT  # real-time (not scaled)
        if sim.rampup_elapsed > 0.5:
            sim.slowmo_phase = "rampup"
            sim.rampup_elapsed = 0.0
            print("[SlowMo] Ramping up...")

    elif sim.slowmo_phase == "rampup":
        sim.rampup_elapsed += DT
        progress = sim.rampup_elapsed / SLOWMO_RAMPUP_DURATION
        # Ease-in curve (starts slow, accelerates)
        eased = progress * progress
        sim.time_scale = SLOWMO_MIN_SCALE + eased * (1.0 - SLOWMO_MIN_SCALE)
        if sim.time_scale >= 1.0:
            sim.time_scale = 1.0
            sim.slowmo_phase = "done"
            print("[SlowMo] Full speed.")


@entrypoint
def main(app: App) -> App:
    app.init_resource(NewtonSim)
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, poll_build)
        .add_systems(Update, handle_input)
        .add_systems(Update, slowmo_controller)
        .add_systems(Update, step_physics)
        .add_systems(Update, sync_transforms)
        .add_systems(Update, animate_embers)
    )


if __name__ == "__main__":
    main().run()
