"""
N-Body Gravity Simulation with JAX

Demonstrates JAX integration with PyBevy's View API for GPU-accelerated
all-pairs gravitational simulation. Each body attracts every other body
(O(n²) interactions), which is trivially parallel on GPU via JAX.

Data flow per frame:
  ECS storage → (pytree copy out) → @jax.jit kernel → (from_jax copy back) → ECS storage

Performance:
  The O(n²) gravity computation dominates; copy overhead is negligible.
  With JAX on GPU, 4096 bodies runs comfortably at 60fps.

Controls:
  - Mouse drag to rotate camera
  - Mouse wheel to zoom
  - Press Ctrl+C to exit
"""

import math
from dataclasses import dataclass, field
from types import SimpleNamespace

try:
    import jax  # type: ignore[import-untyped]
    import jax.numpy as jnp  # type: ignore[import-untyped]
except ImportError:
    print("ERROR: JAX is required for this example. Install with: pip install jax jaxlib")
    print("For GPU support: pip install jax[cuda12]")
    exit(1)

import numpy as np

import pybevy.ecs.jax_ext  # noqa: F401  # activate JAX support
from pybevy.contrib import OrbitCamera, OrbitCameraPlugin
from pybevy.prelude import *

# Configuration
N_BODIES = 512
GRAVITY_STRENGTH = 50.0
SOFTENING = 0.5
SPAWN_RADIUS = 30.0



@component
@dataclass
class Velocity(Component):
    vel: Vec3 = field(default_factory=lambda: Vec3.ZERO)


@component
@dataclass
class Mass(Component):
    value: float = 1.0


@component
class Body(Component):
    pass



@jax.jit
def nbody_step(
    pos: SimpleNamespace,
    vel: SimpleNamespace,
    mass: jnp.ndarray,
    dt: float,
) -> tuple[SimpleNamespace, SimpleNamespace]:
    """All-pairs gravitational interaction.

    pos and vel are SimpleNamespace(x=, y=, z=) from Vec3ViewColumn pytree.
    For N bodies, computes NxN distance matrix and sums forces.
    This is O(n^2) -- exactly the workload where GPU shines.
    """
    # Pairwise distances (N, N)
    dx = pos.x[:, None] - pos.x[None, :]
    dy = pos.y[:, None] - pos.y[None, :]
    dz = pos.z[:, None] - pos.z[None, :]
    r2 = dx**2 + dy**2 + dz**2 + SOFTENING**2
    inv_r3 = r2 ** (-1.5)

    # Gravitational acceleration
    ax = jnp.sum(dx * inv_r3 * mass[None, :], axis=1) * GRAVITY_STRENGTH
    ay = jnp.sum(dy * inv_r3 * mass[None, :], axis=1) * GRAVITY_STRENGTH
    az = jnp.sum(dz * inv_r3 * mass[None, :], axis=1) * GRAVITY_STRENGTH

    # Integrate velocity
    new_vel = SimpleNamespace(
        x=vel.x + ax * dt,
        y=vel.y + ay * dt,
        z=vel.z + az * dt,
    )

    # Integrate position
    new_pos = SimpleNamespace(
        x=pos.x + new_vel.x * dt,
        y=pos.y + new_vel.y * dt,
        z=pos.z + new_vel.z * dt,
    )

    return new_pos, new_vel



def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    sphere_mesh = meshes.add(Sphere(0.3))
    material = materials.add(StandardMaterial(
        base_color=Color.srgb(0.9, 0.6, 0.2),
        emissive=LinearRgba(0.8, 0.4, 0.1, 1.0),
    ))

    rng = np.random.default_rng(42)
    for _ in range(N_BODIES):
        # Random position on sphere surface
        theta = rng.uniform(0, 2 * math.pi)
        phi = rng.uniform(-math.pi / 2, math.pi / 2)
        r = SPAWN_RADIUS * rng.uniform(0.3, 1.0) ** (1 / 3)
        x = r * math.cos(phi) * math.cos(theta)
        y = r * math.cos(phi) * math.sin(theta)
        z = r * math.sin(phi)

        # Tangential velocity for initial orbits
        speed = 3.0 * rng.uniform(0.5, 1.5)
        commands.spawn(
            Body(),
            Mass(value=rng.uniform(0.5, 3.0)),
            Velocity(vel=Vec3(
                -math.sin(theta) * speed,
                math.cos(theta) * speed * 0.3,
                math.cos(phi) * speed * 0.5,
            )),
            Mesh3d(sphere_mesh),
            MeshMaterial3d(material),
            Transform.from_xyz(x, y, z),
        )

    # Lighting
    commands.spawn(
        DirectionalLight(illuminance=5000.0),
        Transform.IDENTITY.looking_at(Vec3(-1.0, -1.0, -1.0), Vec3.Y),
    )
    commands.insert_resource(GlobalAmbientLight(brightness=200.0, color=Color.WHITE))

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, 30.0, 60.0).looking_at(Vec3.ZERO, Vec3.Y),
        OrbitCamera(distance=70.0, yaw=0.0, pitch=0.4, target=Vec3.ZERO),
    )



def gravity_system(
    view: View[tuple[Mut[Transform], Mut[Velocity], Mass], With[Body]],
    time: Res[Time],
) -> None:
    dt = time.delta_secs()
    if dt <= 0.0 or dt > 0.1:
        return

    for batch in view.iter_batches():
        pos = batch.column_mut(Transform)
        vel = batch.column_mut(Velocity)
        mass_col = batch.column(Mass)

        # Vec3ViewColumn passes directly as pytree → SimpleNamespace(x=, y=, z=)
        new_pos, new_vel = nbody_step(
            pos.translation, vel.vel, mass_col.value, dt,  # type: ignore[attr-defined]
        )

        # Write results back to ECS (accepts SimpleNamespace with .x, .y, .z)
        pos.translation.from_jax(new_pos)
        vel.vel.from_jax(new_vel)  # type: ignore[attr-defined]



class FPSCounter:
    def __init__(self) -> None:
        self.frame_count: int = 0


def fps_system(time: Res[Time], counter: Local[FPSCounter]) -> None:
    counter.frame_count += 1
    t = time.elapsed_secs()
    if t > 0 and counter.frame_count % 120 == 0:
        print(f"FPS: ~{counter.frame_count / max(t, 0.001):.1f}  ({N_BODIES} bodies, {N_BODIES * N_BODIES:,} interactions)")


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(OrbitCameraPlugin())  # type: ignore[arg-type]
        .add_systems(Startup, setup)
        .add_systems(Update, (gravity_system, fps_system))
    )


if __name__ == "__main__":
    platform = jax.default_backend()
    print(f"JAX N-Body: {N_BODIES} bodies, {N_BODIES * N_BODIES:,} interactions/frame on {platform.upper()}")
    main().run()
