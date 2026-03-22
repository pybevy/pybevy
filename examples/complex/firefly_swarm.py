"""
Firefly Swarm Synchronization - 3,136 fireflies with 576 lights!

Models the Kuramoto synchronization phenomenon where fireflies naturally sync
their flashing patterns through phase coupling. This is a real phenomenon
observed in nature!

Performance Features:
- Sync & Movement: Query + Numba JIT (parallel processing with numba.prange)
- Lighting & Stats: Query iteration
- Python @component fields extracted to np.ndarray for Numba kernels
- Mean-field Kuramoto model: O(n) instead of O(n^2) pairwise coupling

Smart design: 3,136 entities simulated, 576 PointLights for GPU performance!

Watch the coherence metric rise from ~0% to 80%+ as thousands of
fireflies naturally synchronize into beautiful waves of light!
"""

import math
import random
from dataclasses import dataclass, field

try:
    import numba  # type: ignore[import-untyped]
except ImportError:
    print("ERROR: Numba is required for this example.")
    print("Install with: poetry install --extras numba")
    print("Or: pip install numba")
    exit(1)

import numpy as np

from pybevy.contrib import OrbitCamera, OrbitCameraPlugin
from pybevy.light import GlobalAmbientLight
from pybevy.prelude import *

# Constants
REGION_COUNT = 64  # Number of separate regions (8x8 grid)
FIREFLIES_PER_REGION = 49  # Fireflies per region (7x7 grid)
FIREFLIES_WITH_LIGHTS_PER_REGION = 9  # Only some have PointLights (3x3 subset)
ALPHA_FIREFLIES_PER_REGION = 5  # Influential fireflies
FIREFLY_SPACING = 1.5
NATURAL_FREQUENCY = 2.0  # Base oscillation frequency (Hz)
COUPLING_STRENGTH = 1.2  # How strongly fireflies couple
ALPHA_COUPLING_STRENGTH = 2.5  # Alpha fireflies stronger

# State constants
FIREFLY_DARK = 0
FIREFLY_CHARGING = 1
FIREFLY_FLASHING = 2
FIREFLY_COOLING = 3


@component
@dataclass
class Firefly(Component):
    """Firefly with oscillator phase and state."""
    phase: float  # [0, 2π]
    state: int
    region_idx: int = 0
    coupling_strength: float = COUPLING_STRENGTH
    # Movement parameters
    vel: Vec3 = field(default_factory=lambda: Vec3.ZERO)
    region_center_x: float = 0.0
    region_center_z: float = 0.0
    region_radius: float = 5.0


@component
@dataclass
class AlphaFirefly(Component):
    """Marker for alpha fireflies."""
    region_idx: int = 0


@resource
class SwarmState(Resource):
    """Global swarm state."""
    def __init__(self) -> None:
        self.global_coherence = 0.0
        self.mean_phase_cos = 0.0
        self.mean_phase_sin = 0.0


def setup_swarm(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Set up firefly swarm."""

    # Materials
    ground_material = materials.add(Color.srgb(0.05, 0.1, 0.05))
    firefly_material = materials.add(Color.srgb(0.2, 0.15, 0.1))
    alpha_material = materials.add(Color.srgb(0.3, 0.25, 0.15))

    # Grid dimensions
    grid_size = math.ceil(math.sqrt(FIREFLIES_PER_REGION))
    region_width = grid_size * FIREFLY_SPACING + FIREFLY_SPACING * 2
    region_spacing = region_width + 8.0

    regions_per_row = math.ceil(math.sqrt(REGION_COUNT))
    region_rows = math.ceil(REGION_COUNT / regions_per_row)

    # Meshes
    firefly_mesh = meshes.add(Sphere(0.2))
    ground_mesh = meshes.add(Cuboid(region_width, 0.5, region_width))

    total_fireflies = 0
    total_alphas = 0
    total_lights = 0

    # Create regions
    for region_idx in range(REGION_COUNT):
        region_col = region_idx % regions_per_row
        region_row = region_idx // regions_per_row

        region_offset_x = (region_col - (regions_per_row - 1) / 2) * region_spacing
        region_offset_z = (region_row - (region_rows - 1) / 2) * region_spacing

        # Ground
        commands.spawn(
            Mesh3d(ground_mesh),
            MeshMaterial3d(ground_material),
            Transform.from_xyz(region_offset_x, -0.25, region_offset_z),
        )

        # Fireflies
        alpha_positions = random.sample(range(FIREFLIES_PER_REGION), ALPHA_FIREFLIES_PER_REGION)
        # Select which fireflies get lights (spread throughout region)
        light_positions = set()
        light_stride = int(math.sqrt(FIREFLIES_PER_REGION / FIREFLIES_WITH_LIGHTS_PER_REGION))
        for lrow in range(int(math.sqrt(FIREFLIES_WITH_LIGHTS_PER_REGION))):
            for lcol in range(int(math.sqrt(FIREFLIES_WITH_LIGHTS_PER_REGION))):
                idx = (lrow * light_stride * grid_size) + (lcol * light_stride)
                if idx < FIREFLIES_PER_REGION:
                    light_positions.add(idx)

        for i in range(FIREFLIES_PER_REGION):
            row = i // grid_size
            col = i % grid_size

            x = region_offset_x + (col - (grid_size - 1) / 2) * FIREFLY_SPACING
            z = region_offset_z + (row - (grid_size - 1) / 2) * FIREFLY_SPACING
            y = random.uniform(1.0, 3.5)

            # Distribute initial phases so some start visible
            initial_phase = random.uniform(0, 2 * math.pi)

            # Set initial state based on phase so some are visible immediately
            if initial_phase < math.pi / 2:
                initial_state = FIREFLY_DARK
            elif initial_phase < math.pi:
                initial_state = FIREFLY_CHARGING
            elif initial_phase < 3 * math.pi / 2:
                initial_state = FIREFLY_FLASHING
            else:
                initial_state = FIREFLY_COOLING

            is_alpha = i in alpha_positions
            has_light = i in light_positions

            # Initial velocity for gentle drifting
            vel_x = random.uniform(-0.5, 0.5)
            vel_y = random.uniform(-0.3, 0.3)
            vel_z = random.uniform(-0.5, 0.5)

            region_radius = region_width / 2.5

            # Set initial intensity to match state
            if initial_state == FIREFLY_DARK:
                initial_intensity = 5000.0
            elif initial_state == FIREFLY_CHARGING:
                initial_intensity = 20000.0
            elif initial_state == FIREFLY_FLASHING:
                initial_intensity = 150000.0
            else:  # COOLING
                initial_intensity = 5000.0

            # Create base entity
            entity_components = [
                Mesh3d(firefly_mesh),
                MeshMaterial3d(alpha_material if is_alpha else firefly_material),
                Transform.from_xyz(x, y, z),
                Firefly(
                    phase=initial_phase,
                    state=initial_state,
                    region_idx=region_idx,
                    coupling_strength=ALPHA_COUPLING_STRENGTH if is_alpha else COUPLING_STRENGTH,
                    vel=Vec3(vel_x, vel_y, vel_z),
                    region_center_x=region_offset_x,
                    region_center_z=region_offset_z,
                    region_radius=region_radius,
                ),
            ]

            # Add alpha marker
            if is_alpha:
                entity_components.append(AlphaFirefly(region_idx=region_idx))
                total_alphas += 1

            # Add PointLight only to selected fireflies
            if has_light:
                entity_components.append(
                    PointLight(
                        intensity=initial_intensity,
                        color=Color.srgb(1.0, 0.9, 0.5) if is_alpha else Color.srgb(1.0, 0.95, 0.6),
                        range=50.0,
                        shadows_enabled=False,
                    )
                )
                total_lights += 1

            commands.spawn(*entity_components)
            total_fireflies += 1

    # Ambient light
    commands.insert_resource(GlobalAmbientLight(brightness=20.0, color=Color.srgb(0.1, 0.1, 0.15)))

    # Camera
    total_width = regions_per_row * region_spacing
    camera_distance = max(total_width, region_spacing * region_rows) * 0.8
    camera_height = camera_distance * 0.8
    camera_z = camera_distance * 0.5

    target = Vec3(0.0, 2.0, 0.0)
    initial_pitch = math.atan2(camera_height - 2.0, camera_z)

    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, camera_height, camera_z).looking_at(target, Vec3.Y),
        OrbitCamera(
            distance=math.sqrt((camera_height - 2.0) ** 2 + camera_z**2),
            yaw=0.0,
            pitch=initial_pitch,
            target=target,
        ),
    )

    commands.insert_resource(SwarmState())


@numba.jit(nopython=True)  # type: ignore[misc]
def kuramoto_sync_kernel(
    phase: np.ndarray,  # Firefly.phase - np.ndarray!
    states: np.ndarray,  # Firefly.state - np.ndarray!
    coupling: np.ndarray,  # Firefly.coupling_strength - np.ndarray!
    delta_time: float,
    natural_freq: float,
) -> tuple[float, float]:
    """Kuramoto mean-field synchronization with Numba."""
    n = len(phase)

    # First pass: calculate mean field
    cos_sum = 0.0
    sin_sum = 0.0
    for i in range(n):
        cos_sum += math.cos(phase[i])
        sin_sum += math.sin(phase[i])

    mean_cos = cos_sum / n
    mean_sin = sin_sum / n
    mean_phase = math.atan2(mean_sin, mean_cos)
    coherence = math.sqrt(mean_cos * mean_cos + mean_sin * mean_sin)

    # Second pass: update phases with mean-field coupling
    for i in numba.prange(n):
        # Natural oscillation
        phase[i] += delta_time * natural_freq

        # Kuramoto coupling
        phase_diff = mean_phase - phase[i]
        phase[i] += coupling[i] * math.sin(phase_diff) * delta_time

        # Wrap phase
        while phase[i] > 2 * math.pi:
            phase[i] -= 2 * math.pi
        while phase[i] < 0:
            phase[i] += 2 * math.pi

        # Update state based on phase
        if phase[i] < math.pi / 2:
            states[i] = FIREFLY_DARK
        elif phase[i] < math.pi:
            states[i] = FIREFLY_CHARGING
        elif phase[i] < 3 * math.pi / 2:
            states[i] = FIREFLY_FLASHING
        else:
            states[i] = FIREFLY_COOLING

    return coherence, mean_cos


def firefly_sync_system(
    query: Query[Mut[Firefly]],
    swarm_state: ResMut[SwarmState],
    time: Res[Time],
) -> None:
    """Synchronization using mean-field Kuramoto model with Query+Numba."""
    dt = time.delta_secs()

    # Extract fields to numpy arrays
    fireflies = list(query)
    n = len(fireflies)
    if n == 0:
        return

    phase = np.empty(n, dtype=np.float64)
    states = np.empty(n, dtype=np.int64)
    coupling = np.empty(n, dtype=np.float64)

    for i, firefly in enumerate(fireflies):
        phase[i] = firefly.phase
        states[i] = firefly.state
        coupling[i] = firefly.coupling_strength

    coherence, mean_cos = kuramoto_sync_kernel(
        phase, states, coupling, dt, NATURAL_FREQUENCY,
    )

    # Write back modified fields (use stored list - query iterator is exhausted)
    for i, firefly in enumerate(fireflies):
        firefly.phase = float(phase[i])
        firefly.state = int(states[i])

    swarm_state.global_coherence = coherence
    swarm_state.mean_phase_cos = mean_cos


@numba.jit(nopython=True)  # type: ignore[misc]
def movement_kernel(
    pos,    # Vec3ViewColumn (Transform.translation)
    vel,    # Vec3ViewColumn (Firefly.vel)
    center_x: np.ndarray,  # Firefly.region_center_x
    center_z: np.ndarray,  # Firefly.region_center_z
    radius: np.ndarray,    # Firefly.region_radius
    delta_time: float,
) -> None:
    """Update firefly positions and velocities with Numba."""
    n = len(pos.x)
    damping = 0.98
    max_speed = 1.5

    for i in numba.prange(n):
        # Update position
        pos.x[i] += vel.x[i] * delta_time
        pos.y[i] += vel.y[i] * delta_time
        pos.z[i] += vel.z[i] * delta_time

        # Region boundary steering
        dx = pos.x[i] - center_x[i]
        dz = pos.z[i] - center_z[i]
        dist = math.sqrt(dx * dx + dz * dz)

        if dist > radius[i]:
            steer = 2.0 * delta_time / dist
            vel.x[i] -= dx * steer
            vel.z[i] -= dz * steer

        # Vertical bounds
        if pos.y[i] < 0.5:
            vel.y[i] += 1.0 * delta_time
        elif pos.y[i] > 4.5:
            vel.y[i] -= 1.0 * delta_time

        # Random wandering (simplified - deterministic noise)
        vel.x[i] += math.sin(i + delta_time * 0.5) * 0.02
        vel.y[i] += math.cos(i * 0.7 + delta_time * 0.3) * 0.01
        vel.z[i] += math.sin(i * 1.3 + delta_time * 0.4) * 0.02

        # Damping
        vel.x[i] *= damping
        vel.y[i] *= damping
        vel.z[i] *= damping

        # Speed limit
        speed = math.sqrt(vel.x[i] ** 2 + vel.y[i] ** 2 + vel.z[i] ** 2)
        if speed > max_speed:
            scale = max_speed / speed
            vel.x[i] *= scale
            vel.y[i] *= scale
            vel.z[i] *= scale


def firefly_movement_system(
    view: View[tuple[Mut[Transform], Mut[Firefly]], With[Firefly]],
    time: Res[Time],
) -> None:
    """Make fireflies drift and fly around their regions with View+Numba."""
    dt = time.delta_secs()

    for batch in view.iter_batches():
        pos = batch.column_mut(Transform)
        firefly_col = batch.column_mut(Firefly)
        movement_kernel(
            pos.translation,
            firefly_col.vel,  # type: ignore[attr-defined]
            firefly_col.region_center_x,  # type: ignore[attr-defined]
            firefly_col.region_center_z,  # type: ignore[attr-defined]
            firefly_col.region_radius,  # type: ignore[attr-defined]
            dt,
        )


def firefly_lighting_system(
    query: Query[tuple[Mut[PointLight], Firefly]],
) -> None:
    """Update light intensity based on firefly state."""
    for light, firefly in query:
        if firefly.state == FIREFLY_DARK:
            light.intensity = 0.0
        elif firefly.state == FIREFLY_CHARGING:
            light.intensity = 2000.0
        elif firefly.state == FIREFLY_FLASHING:
            light.intensity = 15000.0
        elif firefly.state == FIREFLY_COOLING:
            light.intensity = 5000.0


def stats_system(
    swarm_state: Res[SwarmState],
    query: Query[Firefly],
    time: Res[Time],
) -> None:
    """Display synchronization statistics."""
    elapsed = int(time.elapsed_secs())
    if elapsed % 3 == 0 and elapsed > 0 and time.delta_secs() < 0.1:
        dark = charging = flashing = cooling = 0
        for firefly in query:
            if firefly.state == FIREFLY_DARK:
                dark += 1
            elif firefly.state == FIREFLY_CHARGING:
                charging += 1
            elif firefly.state == FIREFLY_FLASHING:
                flashing += 1
            elif firefly.state == FIREFLY_COOLING:
                cooling += 1

        coherence_pct = swarm_state.global_coherence * 100
        print(f"Coherence: {coherence_pct:.1f}%")


@entrypoint
def main(app: App) -> App:
    """Create firefly swarm simulation."""
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(OrbitCameraPlugin())
        .add_systems(Startup, setup_swarm)
        .add_systems(
            Update,
            (
                firefly_sync_system,
                firefly_movement_system,
                firefly_lighting_system,
                stats_system,
            ),
        )
    )


if __name__ == "__main__":
    total = REGION_COUNT * FIREFLIES_PER_REGION
    print(f"Firefly swarm: {total:,} fireflies, Kuramoto synchronization. Drag mouse to rotate.")
    main().run()
