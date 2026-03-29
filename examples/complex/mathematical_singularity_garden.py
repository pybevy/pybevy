"""
Mathematical Singularity Garden - An Ultra-Performance Math Visualization

A mesmerizing 3D visualization combining advanced mathematical concepts:
- Morphing parametric surfaces (Klein bottle, Möbius strip, torus knots)
- Quantum probability wave functions creating interference patterns
- Fractal attractors with emergent complexity
- Ferrofluid-inspired magnetic field simulations
- Non-Euclidean geometry transformations

All computed in parallel using ViewColumn + Numba JIT for extreme performance.
"""

import math
from dataclasses import dataclass
from typing import TYPE_CHECKING

try:
    import numba  # type: ignore[import-untyped]
except ImportError:
    print("ERROR: Numba is required for this example. Install with: pip install numba")
    exit(1)

from pybevy.contrib import OrbitCamera, OrbitCameraPlugin
from pybevy.prelude import *

if TYPE_CHECKING:
    from pybevy.ecs import FieldExpr  # type: ignore[assignment]


# Configuration
GRID_SIZE = 200  # 200x200 = 40,000 entities
PARTICLE_SPACING = 2.0
MORPH_SPEED = 0.5
COLOR_CYCLE_SPEED = 1.0



@numba.jit(nopython=True, parallel=True)
def klein_bottle_morph(
    pos_x: "FieldExpr",
    pos_y: "FieldExpr",
    pos_z: "FieldExpr",
    scale_x: "FieldExpr",
    scale_y: "FieldExpr",
    scale_z: "FieldExpr",
    time: float,
    morph_factor: float,
) -> None:
    """Morph particles into a Klein bottle surface with dynamic scaling."""
    n = len(pos_x)

    for i in numba.prange(n):
        # Map grid position to parametric coordinates
        u = (i % GRID_SIZE) / float(GRID_SIZE) * 2.0 * math.pi
        v = (i // GRID_SIZE) / float(GRID_SIZE) * 2.0 * math.pi

        # Klein bottle parametric equations
        a = 4.0 * (1.0 - math.cos(u) / 2.0)

        if u < math.pi:
            x = 6.0 * math.cos(u) * (1.0 + math.sin(u))
            x += a * math.cos(v + math.pi)
            y = 16.0 * math.sin(u)
            y += a * math.sin(v + math.pi)
        else:
            x = 6.0 * math.cos(u) * (1.0 + math.sin(u))
            x += a * math.cos(u) * math.cos(v)
            y = 16.0 * math.sin(u)
            y += a * math.sin(u) * math.cos(v)

        z = a * math.sin(v)

        # Add time-based undulation
        wave = math.sin(u * 3.0 - time * 2.0) * math.cos(v * 2.0 + time)

        # Blend with original position
        orig_x = pos_x[i]
        orig_z = pos_z[i]

        pos_x[i] = orig_x * (1.0 - morph_factor) + x * morph_factor * 5.0
        pos_y[i] = y * morph_factor * 3.0 + wave * 2.0
        pos_z[i] = orig_z * (1.0 - morph_factor) + z * morph_factor * 5.0

        # Dynamic scaling based on position
        scale_factor = 0.3 + abs(wave) * 0.3
        scale_x[i] = scale_factor
        scale_y[i] = scale_factor
        scale_z[i] = scale_factor


@numba.jit(nopython=True, parallel=True)
def quantum_interference_field(
    pos_x: "FieldExpr",
    pos_y: "FieldExpr",
    pos_z: "FieldExpr",
    rot_x: "FieldExpr",
    rot_y: "FieldExpr",
    rot_z: "FieldExpr",
    rot_w: "FieldExpr",
    time: float,
) -> None:
    """Simulate quantum wave function interference patterns."""
    n = len(pos_x)

    # Wave sources (quantum emitters)
    sources = [
        (0.0, 0.0, 1.0, 2.0),  # x, z, frequency, amplitude
        (30.0, 30.0, 1.5, 1.8),
        (-30.0, 30.0, 1.8, 1.6),
        (30.0, -30.0, 2.2, 1.4),
        (-30.0, -30.0, 2.5, 1.2),
    ]

    for i in numba.prange(n):
        x = pos_x[i]
        z = pos_z[i]

        # Superposition of wave functions
        psi_real = 0.0
        psi_imag = 0.0

        for sx, sz, freq, amp in sources:
            dx = x - sx
            dz = z - sz
            r = math.sqrt(dx * dx + dz * dz)

            # Wave function: ψ = A * e^(i(kr - ωt)) / r
            phase = freq * r - time * freq * 2.0

            # Avoid division by zero
            if r > 0.1:
                psi_real += amp * math.cos(phase) / (1.0 + r * 0.02)
                psi_imag += amp * math.sin(phase) / (1.0 + r * 0.02)

        # Probability density: |ψ|²
        probability = psi_real * psi_real + psi_imag * psi_imag

        # Height from probability amplitude
        pos_y[i] = probability * 2.0

        # Rotation based on wave phase
        phase_angle = math.atan2(psi_imag, psi_real)

        # Create quaternion from axis-angle (rotate around Y axis)
        half_angle = phase_angle * 0.5
        rot_x[i] = 0.0
        rot_y[i] = math.sin(half_angle)
        rot_z[i] = 0.0
        rot_w[i] = math.cos(half_angle)


@numba.jit(nopython=True, parallel=True)
def fractal_attractor_field(
    pos_x: "FieldExpr",
    pos_y: "FieldExpr",
    pos_z: "FieldExpr",
    scale_x: "FieldExpr",
    scale_y: "FieldExpr",
    scale_z: "FieldExpr",
    time: float,
) -> None:
    """Generate a 3D fractal attractor field (Rössler-Lorenz hybrid)."""
    n = len(pos_x)

    # Attractor parameters (time-varying for evolution)
    a = 0.2 + math.sin(time * 0.1) * 0.05
    b = 0.2 + math.cos(time * 0.15) * 0.05
    c = 5.7 + math.sin(time * 0.08) * 0.5

    # Lorenz parameters
    sigma = 10.0
    rho = 28.0 - math.sin(time * 0.1) * 5.0
    beta = 8.0 / 3.0

    for i in numba.prange(n):
        # Use grid position as initial condition
        x0 = (pos_x[i] + 100.0) * 0.01 - 1.0
        y0 = 0.0
        z0 = (pos_z[i] + 100.0) * 0.01 - 1.0

        # Iterate the attractor equations
        x, y, z = x0, y0, z0
        iterations = 50

        for _ in range(iterations):
            # Hybrid Rössler-Lorenz dynamics
            dx = -y - z + sigma * (y - x) * 0.01
            dy = x + a * y + x * (rho - z) * 0.01
            dz = b + z * (x - c) + x * y * beta * 0.001

            # Integration step
            dt = 0.01
            x += dx * dt
            y += dy * dt
            z += dz * dt

            # Prevent explosion
            mag = math.sqrt(x * x + y * y + z * z)
            if mag > 50.0:
                x *= 50.0 / mag
                y *= 50.0 / mag
                z *= 50.0 / mag

        # Map attractor position to height and scaling
        height = y * 0.5

        # Fractal detail from iteration count
        detail = math.sqrt(x * x + z * z) * 0.1

        pos_y[i] = height + detail * math.sin(time * 2.0 + i * 0.01)

        # Scale based on attractor density
        scale = 0.2 + min(abs(height) * 0.1, 0.5)
        scale_x[i] = scale
        scale_y[i] = scale * 2.0  # Elongate vertically
        scale_z[i] = scale


@numba.jit(nopython=True, parallel=True)
def ferrofluid_magnetic_field(
    pos_x: "FieldExpr",
    pos_y: "FieldExpr",
    pos_z: "FieldExpr",
    rot_x: "FieldExpr",
    rot_y: "FieldExpr",
    rot_z: "FieldExpr",
    rot_w: "FieldExpr",
    time: float,
) -> None:
    """Simulate ferrofluid behavior in a dynamic magnetic field."""
    n = len(pos_x)

    # Moving magnetic dipoles
    dipole1_x = math.cos(time * 0.8) * 40.0
    dipole1_z = math.sin(time * 0.8) * 40.0
    dipole2_x = math.cos(time * 1.2 + math.pi) * 30.0
    dipole2_z = math.sin(time * 1.2 + math.pi) * 30.0

    for i in numba.prange(n):
        x = pos_x[i]
        z = pos_z[i]

        # Calculate magnetic field from dipoles
        # Dipole 1
        dx1 = x - dipole1_x
        dz1 = z - dipole1_z
        r1_sq = dx1 * dx1 + dz1 * dz1 + 0.1  # Avoid division by zero
        r1 = math.sqrt(r1_sq)

        # Magnetic field components (simplified dipole field)
        Bx1 = 3.0 * dx1 * dz1 / (r1_sq * r1)
        Bz1 = (3.0 * dz1 * dz1 - r1_sq) / (r1_sq * r1)
        By1 = 20.0 / r1  # Vertical component

        # Dipole 2 (opposite polarity)
        dx2 = x - dipole2_x
        dz2 = z - dipole2_z
        r2_sq = dx2 * dx2 + dz2 * dz2 + 0.1
        r2 = math.sqrt(r2_sq)

        Bx2 = -3.0 * dx2 * dz2 / (r2_sq * r2)
        Bz2 = -(3.0 * dz2 * dz2 - r2_sq) / (r2_sq * r2)
        By2 = -15.0 / r2

        # Total field
        Bx = Bx1 + Bx2
        By = By1 + By2
        Bz = Bz1 + Bz2

        # Field magnitude
        B_mag = math.sqrt(Bx * Bx + By * By + Bz * Bz)

        # Ferrofluid spike height proportional to field strength
        spike_height = math.log(1.0 + B_mag) * 3.0

        # Add surface tension effects (smoothing)
        tension = math.sin(x * 0.2 + time) * math.cos(z * 0.2 - time) * 0.5

        pos_y[i] = spike_height + tension

        # Align rotation with magnetic field direction
        if B_mag > 0.01:
            # Normalize field vector
            Bx /= B_mag
            By /= B_mag
            Bz /= B_mag

            # Create quaternion to align Y-axis with field
            # This is a simplified version
            angle = math.acos(min(1.0, max(-1.0, By)))
            axis_x = -Bz
            axis_z = Bx
            axis_mag = math.sqrt(axis_x * axis_x + axis_z * axis_z)

            if axis_mag > 0.001:
                axis_x /= axis_mag
                axis_z /= axis_mag

                half_angle = angle * 0.5
                sin_half = math.sin(half_angle)

                rot_x[i] = axis_x * sin_half
                rot_y[i] = 0.0
                rot_z[i] = axis_z * sin_half
                rot_w[i] = math.cos(half_angle)
            else:
                rot_x[i] = 0.0
                rot_y[i] = 0.0
                rot_z[i] = 0.0
                rot_w[i] = 1.0


@numba.jit(nopython=True, parallel=True)
def hyperbolic_tessellation(
    pos_x: "FieldExpr",
    pos_y: "FieldExpr",
    pos_z: "FieldExpr",
    scale_x: "FieldExpr",
    scale_y: "FieldExpr",
    scale_z: "FieldExpr",
    time: float,
) -> None:
    """Create a hyperbolic tessellation in 3D space (Poincaré disk model)."""
    n = len(pos_x)

    for i in numba.prange(n):
        x = pos_x[i]
        z = pos_z[i]

        # Map to Poincaré disk coordinates
        r = math.sqrt(x * x + z * z)
        theta = math.atan2(z, x)

        # Hyperbolic distance from origin
        if r < 100.0:  # Limit to disk radius
            # Transform to hyperbolic space
            hyp_r = 2.0 * math.atanh(min(0.99, r / 100.0))

            # Tessellation pattern (regular heptagon tiling)
            p = 7  # Heptagon
            q = 3  # Three meet at each vertex

            # Generate tessellation coordinates
            u = hyp_r * math.cos(theta * p + time)
            v = hyp_r * math.sin(theta * p + time)

            # Height based on hyperbolic curvature (constant negative curvature = -1.0)
            height = math.sinh(hyp_r) * math.sin(u * q) * math.cos(v * q) * 2.0

            # Add Escher-like morphing
            morph = math.sin(hyp_r * 2.0 - time * 1.5)

            pos_y[i] = height + morph * 3.0

            # Scale decreases with hyperbolic distance (perspective effect)
            scale = math.exp(-hyp_r * 0.5) * (0.5 + abs(morph) * 0.3)
            scale_x[i] = scale
            scale_y[i] = scale
            scale_z[i] = scale
        else:
            pos_y[i] = 0.0
            scale_x[i] = 0.1
            scale_y[i] = 0.1
            scale_z[i] = 0.1


@numba.jit(nopython=True, parallel=True)
def compute_color_field(
    pos_x: "FieldExpr",
    pos_y: "FieldExpr",
    pos_z: "FieldExpr",
    time: float,
    r_out: "FieldExpr",
    g_out: "FieldExpr",
    b_out: "FieldExpr",
) -> None:
    """Compute dynamic colors based on mathematical functions."""
    n = len(pos_x)

    for i in numba.prange(n):
        x = pos_x[i]
        y = pos_y[i]
        z = pos_z[i]

        # Complex color mapping based on position and height
        # Use cylindrical coordinates for color
        r = math.sqrt(x * x + z * z)
        theta = math.atan2(z, x)

        # Hue from angle and time
        hue = (theta / (2.0 * math.pi) + time * 0.1) % 1.0

        # Saturation from radius
        saturation = 1.0 - math.exp(-r * 0.02)

        # Value from height
        value = 0.5 + 0.5 * math.tanh(y * 0.2)

        # HSV to RGB conversion
        c = value * saturation
        x_hsv = c * (1.0 - abs((hue * 6.0) % 2.0 - 1.0))
        m = value - c

        if hue < 1.0 / 6.0:
            r_out[i] = c + m
            g_out[i] = x_hsv + m
            b_out[i] = m
        elif hue < 2.0 / 6.0:
            r_out[i] = x_hsv + m
            g_out[i] = c + m
            b_out[i] = m
        elif hue < 3.0 / 6.0:
            r_out[i] = m
            g_out[i] = c + m
            b_out[i] = x_hsv + m
        elif hue < 4.0 / 6.0:
            r_out[i] = m
            g_out[i] = x_hsv + m
            b_out[i] = c + m
        elif hue < 5.0 / 6.0:
            r_out[i] = x_hsv + m
            g_out[i] = m
            b_out[i] = c + m
        else:
            r_out[i] = c + m
            g_out[i] = m
            b_out[i] = x_hsv + m



@component
class MathParticle(Component):
    """Marker for mathematical visualization particles."""



@component
@dataclass
class ParticleColor(Component):
    """Dynamic color for particles."""

    r: float = 0.5
    g: float = 0.5
    b: float = 0.5



def setup_scene(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Create materials with different base colors
    material_handles = []
    for _ in range(10):
        # Create metallic materials for more interesting reflections
        material = StandardMaterial(
            base_color=Color.srgba(0.8, 0.8, 0.8, 1.0),
            metallic=0.8,
            perceptual_roughness=0.2,
            reflectance=0.9,
        )
        material_handles.append(materials.add(material))

    # Use spheres for particles (more visually interesting than cubes)
    particle_mesh = meshes.add(Sphere(0.3))

    spawn_count = 0

    for row in range(GRID_SIZE):
        for col in range(GRID_SIZE):
            x = (col - GRID_SIZE / 2) * PARTICLE_SPACING
            z = (row - GRID_SIZE / 2) * PARTICLE_SPACING

            # Choose material based on checkerboard pattern
            mat_idx = ((row // 5) + (col // 5)) % len(material_handles)

            commands.spawn(
                MathParticle(),
                ParticleColor(0.5, 0.5, 0.8),
                Mesh3d(particle_mesh),
                MeshMaterial3d(material_handles[mat_idx]),
                Transform.from_xyz(x, 0.0, z),
            )
            spawn_count += 1


    # Advanced lighting setup for dramatic effect
    # Key light
    commands.spawn(
        DirectionalLight(
            illuminance=15000.0,
            color=Color.srgb(1.0, 0.95, 0.8),
            shadows_enabled=True,
        ),
        Transform.IDENTITY.looking_at(Vec3(-1.0, -2.0, -1.0), Vec3.Y),
    )

    # Fill light
    commands.spawn(
        DirectionalLight(
            illuminance=5000.0,
            color=Color.srgb(0.7, 0.8, 1.0),
            shadows_enabled=False,
        ),
        Transform.IDENTITY.looking_at(Vec3(1.0, -1.0, 1.0), Vec3.Y),
    )

    # Rim light
    commands.spawn(
        DirectionalLight(
            illuminance=8000.0,
            color=Color.srgb(1.0, 0.7, 0.5),
            shadows_enabled=False,
        ),
        Transform.IDENTITY.looking_at(Vec3(0.0, -1.0, 2.0), Vec3.Y),
    )

    # Ambient for base illumination
    commands.insert_resource(
        GlobalAmbientLight(brightness=150.0, color=Color.srgb(0.4, 0.5, 0.7))
    )

    # Camera setup
    camera_distance = GRID_SIZE * PARTICLE_SPACING * 0.7
    camera_height = camera_distance * 0.6
    target = Vec3.ZERO
    initial_pitch = math.atan2(camera_height, camera_distance)

    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, camera_height, camera_distance).looking_at(
            target, Vec3.Y
        ),
        OrbitCamera(
            distance=math.sqrt(camera_height**2 + camera_distance**2),
            yaw=0.0,
            pitch=initial_pitch,
            target=target,
        ),
    )



class AnimationState:
    def __init__(self) -> None:
        self.frame_count: int = 0
        self.current_mode: int = 0


def mathematical_animation_system(
    view: View[Mut[Transform], With[MathParticle]],
    time: Res[Time],
    state: Local[AnimationState],
) -> None:
    t = time.elapsed_secs()
    state.frame_count += 1

    # Cycle through different mathematical visualizations
    mode_duration = 12.0  # Seconds per mode
    mode = int(t / mode_duration) % 6

    # Calculate smooth transition factor
    mode_time = t % mode_duration
    transition_time = 2.0  # Seconds for transition
    morph_factor = mode_time / transition_time if mode_time < transition_time else 1.0

    # Announce mode changes
    if mode != state.current_mode:
        state.current_mode = mode
        modes = [
            "Klein Bottle Manifold",
            "Quantum Interference Patterns",
            "Fractal Attractor Field",
            "Ferrofluid Magnetic Simulation",
            "Hyperbolic Tessellation",
            "Combined Singularity",
        ]
        print(f"Mode: {modes[mode]}")

    for batch in view.iter_batches():
        transform = batch.column_mut(Transform)

        if mode == 0:
            # Klein bottle
            klein_bottle_morph(
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                transform.scale.x,
                transform.scale.y,
                transform.scale.z,
                t,
                morph_factor,
            )
        elif mode == 1:
            # Quantum interference
            quantum_interference_field(
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                transform.rotation.x,
                transform.rotation.y,
                transform.rotation.z,
                transform.rotation.w,
                t,
            )
        elif mode == 2:
            # Fractal attractor
            fractal_attractor_field(
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                transform.scale.x,
                transform.scale.y,
                transform.scale.z,
                t,
            )
        elif mode == 3:
            # Ferrofluid
            ferrofluid_magnetic_field(
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                transform.rotation.x,
                transform.rotation.y,
                transform.rotation.z,
                transform.rotation.w,
                t,
            )
        elif mode == 4:
            # Hyperbolic tessellation
            hyperbolic_tessellation(
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                transform.scale.x,
                transform.scale.y,
                transform.scale.z,
                t,
            )
        else:
            # Combined mode - blend multiple effects
            # Run quantum interference at half strength
            quantum_interference_field(
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                transform.rotation.x,
                transform.rotation.y,
                transform.rotation.z,
                transform.rotation.w,
                t,
            )

            # Then modulate with fractal field
            y_temp = transform.translation.y
            for i in range(len(y_temp)):  # type: ignore[arg-type]
                current = y_temp.peek(i)  # type: ignore[attr-defined]
                y_temp[i] = current * 0.5  # type: ignore[index]

            # Add ferrofluid on top
            ferrofluid_magnetic_field(
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                transform.rotation.x,
                transform.rotation.y,
                transform.rotation.z,
                transform.rotation.w,
                t * 0.5,
            )


class FPSCounter:
    def __init__(self) -> None:
        self.frame_count: int = 0


def performance_monitor_system(time: Res[Time], counter: Local[FPSCounter]) -> None:
    counter.frame_count += 1
    if counter.frame_count % 300 == 0 and counter.frame_count > 0:
        elapsed = time.elapsed_secs()
        fps = counter.frame_count / elapsed
        particles = GRID_SIZE * GRID_SIZE

        print(f"FPS: {fps:.1f} | Particles: {particles:,}")


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(OrbitCameraPlugin)
        .add_systems(Startup, setup_scene)
        .add_systems(
            Update,
            (
                mathematical_animation_system,
                performance_monitor_system,
            ),
        )
    )


if __name__ == "__main__":
    print(f"Mathematical Singularity Garden: {GRID_SIZE * GRID_SIZE:,} particles. Drag mouse to rotate.")
    main().run()
