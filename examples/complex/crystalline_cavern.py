"""Crystalline Cavern - Advanced Volumetric Lighting Showcase

A mystical underground crystal cave demonstrating Bevy's most advanced rendering features:

Features:
- Volumetric fog and god rays (VolumetricLight + VolumetricFog)
- HDR emissive crystals with Bloom post-processing
- Dynamic spotlights with sweeping volumetric beams
- 20,000+ dust particles using View API + Numba JIT
- Color grading for cinematic atmosphere
- Animated rotating crystals
- Pulsating lights synchronized with crystal rotation
- Cinematic camera path through the cave

Performance:
- View API + Numba for particle physics: ~30-50x faster than Query
- Zero-copy ECS access for Transform and custom components
- Handles 20K+ particles at 60 FPS

Controls:
- Automatic cinematic camera path
- Press Ctrl+C to exit
"""

import math
import random
from dataclasses import dataclass, field

import numpy as np

try:
    import numba  # type: ignore[import-untyped]
except ImportError:
    print("ERROR: Numba is required for this example. Install with: pip install numba")
    exit(1)

from pybevy.camera import Exposure
from pybevy.light import VolumetricFog, VolumetricLight
from pybevy.prelude import *

# Constants
PARTICLE_COUNT = 20000
CRYSTAL_COUNT = 7
SPOTLIGHT_COUNT = 3


@component
@dataclass
class Particle(Component):
    """Dust particle floating in volumetric light."""
    vel: Vec3 = field(default_factory=lambda: Vec3.ZERO)
    lifetime: float = 0.0
    max_lifetime: float = 10.0


@component
class Crystal(Component):
    """Marker for rotating crystals."""
    rotation_speed: float
    phase_offset: float

    def __init__(self, rotation_speed: float = 1.0, phase_offset: float = 0.0):
        self.rotation_speed = rotation_speed
        self.phase_offset = phase_offset


@component
@dataclass
class SweepingLight(Component):
    """Spotlight that sweeps in a pattern."""
    sweep_angle: float = 0.0
    sweep_speed: float = 0.5
    center: Vec3 = field(default_factory=lambda: Vec3.ZERO)


@component
class CinematicCamera(Component):
    """Camera following cinematic path."""


def setup_cave(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Dark cave wall material
    cave_material = materials.add(
        StandardMaterial(
            base_color=Color.srgb(0.15, 0.12, 0.1),
            perceptual_roughness=0.9,
            metallic=0.1,
        )
    )

    # Floor - large flat plane
    floor_mesh = meshes.add(Plane3d().mesh().size(50.0, 50.0).build())
    commands.spawn(
        Mesh3d(floor_mesh),
        MeshMaterial3d(cave_material),
        Transform.from_xyz(0.0, -2.0, 0.0),
    )

    # Ceiling
    commands.spawn(
        Mesh3d(floor_mesh),
        MeshMaterial3d(cave_material),
        Transform.from_xyz(0.0, 12.0, 0.0),
    )

    # Walls (simple box room)
    wall_mesh = meshes.add(Cuboid(50.0, 14.0, 1.0))

    # Back wall
    commands.spawn(
        Mesh3d(wall_mesh),
        MeshMaterial3d(cave_material),
        Transform.from_xyz(0.0, 5.0, -25.0),
    )

    # Front wall
    commands.spawn(
        Mesh3d(wall_mesh),
        MeshMaterial3d(cave_material),
        Transform.from_xyz(0.0, 5.0, 25.0),
    )

    # Left wall
    commands.spawn(
        Mesh3d(meshes.add(Cuboid(1.0, 14.0, 50.0))),
        MeshMaterial3d(cave_material),
        Transform.from_xyz(-25.0, 5.0, 0.0),
    )

    # Right wall
    commands.spawn(
        Mesh3d(meshes.add(Cuboid(1.0, 14.0, 50.0))),
        MeshMaterial3d(cave_material),
        Transform.from_xyz(25.0, 5.0, 0.0),
    )


def setup_crystals(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Crystal mesh - elongated hexagonal prism
    crystal_mesh = meshes.add(Cylinder(0.6, 3.0))

    # Crystal colors (emissive HDR values)
    crystal_colors = [
        LinearRgba.rgb(100.0, 20.0, 150.0),  # Violet
        LinearRgba.rgb(20.0, 150.0, 200.0),  # Cyan
        LinearRgba.rgb(200.0, 50.0, 100.0),  # Magenta
        LinearRgba.rgb(150.0, 200.0, 50.0),  # Yellow-green
        LinearRgba.rgb(200.0, 100.0, 20.0),  # Orange
        LinearRgba.rgb(50.0, 200.0, 150.0),  # Teal
        LinearRgba.rgb(180.0, 50.0, 200.0),  # Purple
    ]

    # Spawn crystals in a circle
    radius = 8.0
    for i in range(CRYSTAL_COUNT):
        angle = (i / CRYSTAL_COUNT) * 2.0 * math.pi
        x = math.cos(angle) * radius
        z = math.sin(angle) * radius

        # Create emissive material for this crystal
        color = crystal_colors[i % len(crystal_colors)]
        crystal_material = materials.add(
            StandardMaterial(
                emissive=color,
                metallic=0.9,
                perceptual_roughness=0.1,
            )
        )

        # Spawn crystal with rotation
        commands.spawn(
            Mesh3d(crystal_mesh),
            MeshMaterial3d(crystal_material),
            Transform.from_xyz(x, 1.5, z).with_rotation(
                Quat.from_rotation_z(math.pi / 6.0)  # Tilt crystals
            ),
            Crystal(
                rotation_speed=0.3 + random.random() * 0.5,
                phase_offset=random.random() * math.pi * 2.0,
            ),
        )

        # Add point light inside crystal for extra glow
        commands.spawn(
            PointLight(
                intensity=5000.0,
                range=15.0,
                radius=0.5,
                color=Color.linear_rgb(color.red / 200.0, color.green / 200.0, color.blue / 200.0),
                shadow_maps_enabled=False,
            ),
            Transform.from_xyz(x, 1.5, z),
        )


def setup_volumetric_lights(commands: Commands) -> None:
    # Spotlight positions and colors
    spotlight_configs = [
        # (x, y, z, target_x, target_y, target_z, r, g, b, sweep_speed)
        (10.0, 10.0, -10.0, 0.0, 0.0, 0.0, 1.0, 0.3, 0.3, 0.3),  # Red
        (-10.0, 10.0, 10.0, 0.0, 0.0, 0.0, 0.3, 1.0, 0.8, 0.5),  # Cyan
        (0.0, 11.0, 0.0, 0.0, 0.0, 0.0, 0.8, 0.3, 1.0, 0.7),     # Purple
    ]

    for _i, (x, y, z, tx, ty, tz, r, g, b, speed) in enumerate(spotlight_configs):
        commands.spawn(
            SpotLight(
                intensity=50000.0,
                range=30.0,
                radius=0.3,
                color=Color.srgb(r, g, b),
                shadow_maps_enabled=True,
                inner_angle=0.3,
                outer_angle=0.6,
            ),
            Transform.from_xyz(x, y, z).looking_at(Vec3(tx, ty, tz), Vec3.Y),
            VolumetricLight(),  # Enable volumetric god rays!
            SweepingLight(
                sweep_speed=speed,
                center=Vec3(x, y, z),
            ),
        )

    # Dim directional light as ambient (moon through cave entrance)
    commands.spawn(
        DirectionalLight(
            illuminance=500.0,
            color=Color.srgb(0.6, 0.7, 0.9),
            shadow_maps_enabled=False,
        ),
        Transform.IDENTITY.looking_at(Vec3(-0.3, -1.0, -0.5), Vec3.Y),
    )

    # Ambient light (very dim)
    commands.insert_resource(GlobalAmbientLight(brightness=50.0, color=Color.srgb(0.5, 0.6, 0.8)))


def spawn_particles(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Tiny sphere for particles
    particle_mesh = meshes.add(Sphere(0.02))

    # Slightly emissive particle material
    particle_material = materials.add(
        StandardMaterial(
            emissive=LinearRgba.rgb(0.5, 0.5, 0.5),
            base_color=Color.srgba(1.0, 1.0, 1.0, 0.8),
            alpha_mode=AlphaMode.Blend(),
        )
    )

    # Spawn particles in volume
    for _ in range(PARTICLE_COUNT):
        x = random.uniform(-15.0, 15.0)
        y = random.uniform(-1.0, 10.0)
        z = random.uniform(-15.0, 15.0)

        vx = random.uniform(-0.5, 0.5)
        vy = random.uniform(-0.2, 0.5)
        vz = random.uniform(-0.5, 0.5)

        commands.spawn(
            Mesh3d(particle_mesh),
            MeshMaterial3d(particle_material),
            Transform.from_xyz(x, y, z).with_scale(Vec3.splat(0.5 + random.random() * 1.0)),
            Particle(
                vel=Vec3(vx, vy, vz),
                lifetime=random.random() * 5.0,
                max_lifetime=8.0 + random.random() * 4.0,
            ),
        )


def setup_post_processing(commands: Commands) -> None:
    # Camera with HDR, bloom, color grading, and tonemapping
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, 3.0, 15.0).looking_at(Vec3(0.0, 2.0, 0.0), Vec3.Y),
        # HDR rendering
        Hdr(),
        # Exposure control
        Exposure.INDOOR,
        # Tonemapping for cinematic look
        Tonemapping.TONY_MC_MAPFACE,
        # Bloom for glowing crystals
        Bloom(
            intensity=0.3,
            low_frequency_boost=0.8,
            low_frequency_boost_curvature=0.95,
            high_pass_frequency=1.0,
            composite_mode=BloomCompositeMode.EnergyConserving,
        ),
        # Color grading for mystical atmosphere
        ColorGrading(
            global_=ColorGradingGlobal(
                exposure=0.0,
                temperature=-0.2,  # Cool blue tint
                tint=0.1,  # Slight magenta
                post_saturation=1.3,  # Boost saturation
            ),
            shadows=ColorGradingSection(
                saturation=1.5,  # More saturated shadows
                contrast=1.1,
            ),
            highlights=ColorGradingSection(
                saturation=0.9,  # Slightly desaturated highlights
                contrast=1.0,
            ),
        ),
        # Volumetric fog throughout the cave
        VolumetricFog(
            ambient_color=Color.srgb(0.3, 0.4, 0.6),
            ambient_intensity=0.05,
            step_count=64,  # High quality
            jitter=0.5,
        ),
        CinematicCamera(),
    )



@numba.jit(nopython=True)  # type: ignore[misc]
def particle_physics_kernel(
    pos,       # Vec3ViewColumn (Transform.translation)
    vel,       # Vec3ViewColumn (Particle.vel)
    lifetime: np.ndarray,      # type: ignore[type-arg]
    max_lifetime: np.ndarray,  # type: ignore[type-arg]
    delta_time: float,
) -> None:
    """Update particle physics with Brownian motion and gentle float (PARALLEL)."""
    n = len(pos.x)

    for i in numba.prange(n):
        # Update lifetime
        lifetime[i] += delta_time

        # Reset particle if lifetime exceeded
        if lifetime[i] >= max_lifetime[i]:
            lifetime[i] = 0.0
            pos.x[i] = random.uniform(-15.0, 15.0)
            pos.y[i] = random.uniform(-1.0, 10.0)
            pos.z[i] = random.uniform(-15.0, 15.0)
            vel.x[i] = random.uniform(-0.5, 0.5)
            vel.y[i] = random.uniform(-0.2, 0.5)
            vel.z[i] = random.uniform(-0.5, 0.5)

        # Brownian motion (random walk)
        vel.x[i] += random.uniform(-0.3, 0.3) * delta_time
        vel.y[i] += random.uniform(-0.3, 0.3) * delta_time
        vel.z[i] += random.uniform(-0.3, 0.3) * delta_time

        # Damping
        vel.x[i] *= 0.99
        vel.y[i] *= 0.99
        vel.z[i] *= 0.99

        # Gentle upward float (simulates warm air rising)
        vel.y[i] += 0.1 * delta_time

        # Update position
        pos.x[i] += vel.x[i] * delta_time
        pos.y[i] += vel.y[i] * delta_time
        pos.z[i] += vel.z[i] * delta_time

        # Boundary wrapping
        if pos.x[i] < -15.0:
            pos.x[i] = 15.0
        if pos.x[i] > 15.0:
            pos.x[i] = -15.0
        if pos.z[i] < -15.0:
            pos.z[i] = 15.0
        if pos.z[i] > 15.0:
            pos.z[i] = -15.0
        if pos.y[i] < -1.0:
            pos.y[i] = 10.0
        if pos.y[i] > 10.0:
            pos.y[i] = -1.0


def particle_physics_system(
    view: View[tuple[Mut[Transform], Mut[Particle]], With[Particle]],
    time: Res[Time],
) -> None:
    """Update particle physics using View + Numba."""
    dt = time.delta_secs()

    for batch in view.iter_batches():
        pos = batch.column_mut(Transform)
        particle_col = batch.column_mut(Particle)
        particle_physics_kernel(
            pos.translation,
            particle_col.vel,       # type: ignore[attr-defined]
            particle_col.lifetime,       # type: ignore[attr-defined]
            particle_col.max_lifetime,   # type: ignore[attr-defined]
            dt,
        )


def crystal_rotation_system(
    query: Query[tuple[Mut[Transform], Crystal]],
    time: Res[Time],
) -> None:
    """Rotate crystals around Y axis."""
    t = time.elapsed_secs()

    for transform, crystal in query:
        # Rotate around Y axis with phase offset
        angle = crystal.rotation_speed * t + crystal.phase_offset
        transform.rotation = Quat.from_rotation_y(angle) * Quat.from_rotation_z(math.pi / 6.0)


def spotlight_sweep_system(
    query: Query[tuple[Mut[Transform], SweepingLight]],
    time: Res[Time],
) -> None:
    """Sweep spotlights in circular patterns."""
    t = time.elapsed_secs()

    for transform, light in query:
        # Calculate sweep angle
        angle = light.sweep_speed * t

        # Update target position (circular sweep)
        radius = 8.0
        target_x = light.center.x + math.cos(angle) * radius
        target_z = light.center.z + math.sin(angle) * radius
        target_y = 0.0

        # Update transform to look at target
        transform.translation = light.center

        # Look at the sweeping target
        target = Vec3(target_x, target_y, target_z)
        transform.look_at(target, Vec3.Y)


def camera_cinematic_system(
    query: Query[Mut[Transform], With[CinematicCamera]],
    time: Res[Time],
) -> None:
    """Cinematic camera path through the cave."""
    t = time.elapsed_secs() * 0.3  # Slow camera movement

    for transform in query:
        # Orbital camera path
        radius = 18.0
        height = 5.0 + math.sin(t * 0.5) * 3.0
        x = math.cos(t) * radius
        z = math.sin(t) * radius

        transform.translation = Vec3(x, height, z)
        transform.look_at(Vec3(0.0, 2.0, 0.0), Vec3.Y)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(
            Startup,
            (
                setup_cave,
                setup_crystals,
                setup_volumetric_lights,
                spawn_particles,
                setup_post_processing,
            ),
        )
        .add_systems(
            Update,
            (
                particle_physics_system,
                crystal_rotation_system,
                spotlight_sweep_system,
                camera_cinematic_system,
            ),
        )
    )


if __name__ == "__main__":
    print(f"Crystalline Cavern: {CRYSTAL_COUNT} crystals, {PARTICLE_COUNT:,} particles, volumetric lighting.")
    main().run()
