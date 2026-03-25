"""Batch spawn + View API demo: 5000 glowing spheres orbiting in a vortex."""

import math
from dataclasses import dataclass

import numpy as np

from pybevy.prelude import *

N = 64000

@component
@dataclass
class Orbit(Component):
    radius: float = 1.0
    speed: float = 1.0
    phase: float = 0.0
    y_speed: float = 0.0


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    rng = np.random.default_rng(42)

    # Spawn camera with bloom
    commands.spawn(
        Camera3d(),
        Tonemapping.TONY_MC_MAPFACE,
        Bloom.NATURAL,
        Transform.from_xyz(0.0, 12.0, 25.0).looking_at(Vec3.ZERO, Vec3.Y),
    )

    # Dim ambient + directional light
    commands.spawn(DirectionalLight(illuminance=200.0), Transform.from_xyz(5.0, 10.0, 5.0))

    # Batch spawn orbiting particles
    radii = rng.uniform(2, 15, N).astype(np.float32)
    phases = rng.uniform(0, 2 * math.pi, N).astype(np.float32)
    speeds = rng.uniform(0.3, 2.0, N).astype(np.float32)
    y_speeds = rng.uniform(0.1, 0.5, N).astype(np.float32)

    pos = np.zeros((N, 3), dtype=np.float32)
    pos[:, 0] = radii * np.cos(phases)
    pos[:, 1] = rng.uniform(-5, 5, N).astype(np.float32)
    pos[:, 2] = radii * np.sin(phases)

    s = 0.01
    commands.spawn_batch(
        Transform.from_numpy(
            translation=pos,
            scale=np.full((N, 3), s, dtype=np.float32),
        ),
        Orbit.from_numpy(radius=radii, speed=speeds, phase=phases, y_speed=y_speeds),
        Mesh3d(meshes.add(Sphere(1.0))),
        MeshMaterial3d(materials.add(StandardMaterial(
            emissive=LinearRgba.rgb(2.0, 0.6, 3.0),
            base_color=Color.srgb(0.8, 0.2, 1.0),
        ))),
    )


def animate(
    view: View[tuple[Mut[Transform], Orbit], With[Orbit]],
    time: Res[Time],
) -> None:
    t = time.elapsed_secs()
    pos = view.column_mut(Transform)
    orb = view.column(Orbit)

    angle = orb.phase + orb.speed * t
    pos.translation.x = orb.radius * angle.cos()
    pos.translation.z = orb.radius * angle.sin()
    pos.translation.y = (orb.y_speed * t + orb.phase).sin() * 4.0


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, animate)
    )


if __name__ == "__main__":
    main().run()
