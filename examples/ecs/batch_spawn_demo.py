"""Batch spawn demo: bouncing balls using spawn_batch + View API."""
from dataclasses import dataclass, field

import numpy as np

from pybevy.prelude import *

BALL_COUNT = 16_000
BALL_R = 0.15
GRAVITY = -9.81
RESTITUTION = 0.7
WALL_LIMIT = 25.0

@component
class Ball(Component):
    pass

@component
@dataclass
class Velocity(Component):
    vel: Vec3 = field(default_factory=lambda: Vec3.ZERO)


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0, 20, 40).looking_at(Vec3(0, 5, 0), Vec3.Y),
        Bloom(intensity=0.1),
        DistanceFog(
            color=Color.srgb(0.05, 0.05, 0.1),
            falloff=FogFalloff.Linear(start=30.0, end=80.0),
        ),
    )

    # Lights
    commands.spawn(
        DirectionalLight(
            illuminance=8000.0,
            shadows_enabled=True,
            color=Color.srgb(1.0, 0.95, 0.85),
        ),
        Transform.IDENTITY.looking_to(Vec3(-1.0, -2.0, -1.5), Vec3.Y),
    )
    commands.insert_resource(GlobalAmbientLight(color=Color.WHITE, brightness=400.0))

    # Ground plane
    ground_mesh = meshes.add(Plane3d(Vec3.Y, half_size=Vec2(30.0, 30.0)))
    ground_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.25, 0.28, 0.30),
    ))
    commands.spawn(Mesh3d(ground_mesh), MeshMaterial3d(ground_mat), Transform.IDENTITY)

    # Batch spawn 5,000 balls
    rng = np.random.default_rng(42)

    positions = np.zeros((BALL_COUNT, 3), dtype=np.float32)
    positions[:, 0] = rng.uniform(-WALL_LIMIT, WALL_LIMIT, BALL_COUNT)  # X
    positions[:, 1] = rng.uniform(5.0, 30.0, BALL_COUNT)                # Y
    positions[:, 2] = rng.uniform(-WALL_LIMIT, WALL_LIMIT, BALL_COUNT)  # Z

    scales = np.full((BALL_COUNT, 3), BALL_R, dtype=np.float32)

    ball_mesh = meshes.add(Sphere(1.0))  # unit sphere, scaled by transform
    ball_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.9, 0.3, 0.15),
        emissive=LinearRgba.rgb(2.0, 0.5, 0.1),
    ))

    commands.spawn_batch(
        Transform.from_numpy(translation=positions, scale=scales),
        Mesh3d(ball_mesh),
        MeshMaterial3d(ball_mat),
        Ball(),
        Velocity(),
    )



def bounce_physics(
    view: View[tuple[Mut[Transform], Mut[Velocity]], With[Ball]],
    time: Res[Time],
) -> None:
    dt = time.delta_secs()
    if dt <= 0.0 or dt > 0.1:
        return

    pos = view.column_mut(Transform)
    vel = view.column_mut(Velocity)

    # Gravity
    vel.vel.y += GRAVITY * dt

    # Integrate position
    pos.translation.x += vel.vel.x * dt
    pos.translation.y += vel.vel.y * dt
    pos.translation.z += vel.vel.z * dt

    # Floor bounce
    hit_floor = pos.translation.y < BALL_R
    vel.vel.y = hit_floor.where(-vel.vel.y * RESTITUTION, vel.vel.y)
    pos.translation.y = pos.translation.y.max(BALL_R)

    # Wall bounce X
    hit_xn = pos.translation.x < -WALL_LIMIT
    hit_xp = pos.translation.x > WALL_LIMIT
    vel.vel.x = (hit_xn | hit_xp).where(-vel.vel.x * RESTITUTION, vel.vel.x)
    pos.translation.x = pos.translation.x.clamp(-WALL_LIMIT, WALL_LIMIT)

    # Wall bounce Z
    hit_zn = pos.translation.z < -WALL_LIMIT
    hit_zp = pos.translation.z > WALL_LIMIT
    vel.vel.z = (hit_zn | hit_zp).where(-vel.vel.z * RESTITUTION, vel.vel.z)
    pos.translation.z = pos.translation.z.clamp(-WALL_LIMIT, WALL_LIMIT)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, bounce_physics)
    )


if __name__ == "__main__":
    main().run()
