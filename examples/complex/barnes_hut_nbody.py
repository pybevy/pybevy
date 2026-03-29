"""
Barnes-Hut N-body Simulation - Two Colliding Star Clusters

Simulates gravitational interactions using the Barnes-Hut algorithm:
- O(N log N) complexity via hierarchical octree approximation
- Two galaxies of configurable size
- Real-time physics with Numba JIT parallel processing
- Query API for ECS access with numpy array extraction

Performance Features:
- Octree construction: O(N log N) spatial partitioning
- Force calculation: Barnes-Hut approximation
- Parallel tree traversal with numba.prange
- Leapfrog integration for numerical stability

Physical Setup:
- Two Plummer sphere galaxies positioned apart
- Initial velocities create head-on collision
- Realistic gravitational softening
"""

import math
import random
from dataclasses import dataclass, field

try:
    import numba  # type: ignore[import-untyped]
except ImportError:
    print("ERROR: Numba is required for this example. Install with: pip install numba")
    exit(1)

import numpy as np

from pybevy.contrib import OrbitCamera, OrbitCameraPlugin
from pybevy.prelude import *

N_PARTICLES = 8 * 1024  # 96k particles
N_PER_GALAXY = N_PARTICLES // 2
GALAXY_SEPARATION = 50.0  # Distance between galaxy centers (closer = faster collision!)
GALAXY_MASS = 10000.0  # Total mass per galaxy (strong gravity for dramatic motion)
PARTICLE_MASS = GALAXY_MASS / N_PER_GALAXY
PLUMMER_RADIUS = 5.0  # Scale radius for Plummer sphere (compact, dense galaxies)
COLLISION_VELOCITY = 20.0  # Initial velocity towards each other (high-speed collision!)

# Barnes-Hut parameters
THETA = 0.7  # Opening angle criterion (higher = faster, good enough for visuals)
SOFTENING = 0.3  # Gravitational softening length (prevents singularities)
G = 20.0  # Gravitational constant (strong forces = visible motion!)
DT = 0.003  # Time step (smaller for stability with strong forces)

# Octree parameters
MAX_PARTICLES_PER_LEAF = 8
MAX_TREE_DEPTH = 20
TREE_ROOT_SIZE = 200.0  # Size of root octree node


@component
@dataclass
class Particle(Component):
    """N-body particle with position, velocity, and mass."""

    pos: Vec3
    vel: Vec3
    accel: Vec3 = field(default_factory=lambda: Vec3.ZERO)
    mass: float = PARTICLE_MASS
    galaxy_id: int = 0


@component
class SimMarker(Component):
    """Marker component for all simulation particles."""



@resource
class SimulationState(Resource):
    """Global simulation state and octree data."""

    def __init__(self) -> None:
        # Octree data (pre-allocated arrays)
        self.max_nodes = N_PARTICLES * 2  # Overestimate
        self.node_count = 0

        # Node data (linearized octree)
        self.node_center_x = np.zeros(self.max_nodes, dtype=np.float32)
        self.node_center_y = np.zeros(self.max_nodes, dtype=np.float32)
        self.node_center_z = np.zeros(self.max_nodes, dtype=np.float32)
        self.node_size = np.zeros(self.max_nodes, dtype=np.float32)
        self.node_mass = np.zeros(self.max_nodes, dtype=np.float32)
        self.node_com_x = np.zeros(self.max_nodes, dtype=np.float32)
        self.node_com_y = np.zeros(self.max_nodes, dtype=np.float32)
        self.node_com_z = np.zeros(self.max_nodes, dtype=np.float32)
        self.node_particle_count = np.zeros(self.max_nodes, dtype=np.int32)
        self.node_is_leaf = np.zeros(self.max_nodes, dtype=np.bool_)
        self.node_children = np.zeros(
            (self.max_nodes, 8), dtype=np.int32
        )  # 8 children per node

        # Particle to node mapping
        self.particle_node_ids = np.zeros(N_PARTICLES, dtype=np.int32)

        # Statistics
        self.total_energy = 0.0
        self.kinetic_energy = 0.0
        self.potential_energy = 0.0
        self.frame_count = 0


def sample_plummer_sphere(
    n: int, scale_radius: float, galaxy_id: int
) -> list[tuple[float, float, float, float, float, float, int]]:
    """Generate particles following Plummer sphere distribution.

    Returns list of (x, y, z, vx, vy, vz, galaxy_id).
    """
    particles = []

    for _ in range(n):
        # Sample radius from Plummer profile: rho(r) proportional to (1 + r^2/a^2)^(-5/2)
        # Using inverse transform sampling
        xi = random.random()
        r = scale_radius / math.sqrt(xi ** (-2.0 / 3.0) - 1.0)

        # Random direction on sphere
        theta = random.uniform(0, 2 * math.pi)
        phi = math.acos(random.uniform(-1, 1))

        x = r * math.sin(phi) * math.cos(theta)
        y = r * math.sin(phi) * math.sin(theta)
        z = r * math.cos(phi)

        # Velocity: isotropic with magnitude based on virial theorem
        # v_escape = sqrt(2 * G * M / r) for Plummer sphere
        v_scale = math.sqrt(G * GALAXY_MASS / (r + scale_radius))
        v_mag = v_scale * random.uniform(0.3, 0.7)

        theta_v = random.uniform(0, 2 * math.pi)
        phi_v = math.acos(random.uniform(-1, 1))

        vx = v_mag * math.sin(phi_v) * math.cos(theta_v)
        vy = v_mag * math.sin(phi_v) * math.sin(theta_v)
        vz = v_mag * math.cos(phi_v)

        particles.append((x, y, z, vx, vy, vz, galaxy_id))

    return particles


def setup_simulation(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Initialize two colliding galaxies."""
    print(f"Setting up {N_PARTICLES:,} particles ({N_PER_GALAXY:,} per galaxy)...")

    # Create shared mesh (very small sphere for performance with 96k particles)
    sphere_mesh = meshes.add(Sphere(radius=0.06))

    # Glowing emissive materials for dramatic visuals!
    galaxy1_mat = materials.add(
        StandardMaterial(
            base_color=Color.srgb(0.3, 0.5, 1.0),  # Bright blue
            emissive=Color.srgb(0.2, 0.4, 2.0),  # Strong blue glow!
        )
    )
    galaxy2_mat = materials.add(
        StandardMaterial(
            base_color=Color.srgb(1.0, 0.4, 0.2),  # Bright red/orange
            emissive=Color.srgb(2.0, 0.3, 0.1),  # Strong red/orange glow!
        )
    )

    # Galaxy 1: Left side, moving right
    galaxy1 = sample_plummer_sphere(N_PER_GALAXY, PLUMMER_RADIUS, 0)
    for x, y, z, vx, vy, vz, gid in galaxy1:
        commands.spawn(
            Particle(
                pos=Vec3(x - GALAXY_SEPARATION / 2, y, z),
                vel=Vec3(vx + COLLISION_VELOCITY, vy, vz),
                galaxy_id=gid,
            ),
            SimMarker(),
            Transform.from_xyz(x - GALAXY_SEPARATION / 2, y, z),
            Mesh3d(sphere_mesh),
            MeshMaterial3d(galaxy1_mat),
        )

    # Galaxy 2: Right side, moving left
    galaxy2 = sample_plummer_sphere(N_PER_GALAXY, PLUMMER_RADIUS, 1)
    for x, y, z, vx, vy, vz, gid in galaxy2:
        commands.spawn(
            Particle(
                pos=Vec3(x + GALAXY_SEPARATION / 2, y, z),
                vel=Vec3(vx - COLLISION_VELOCITY, vy, vz),
                galaxy_id=gid,
            ),
            SimMarker(),
            Transform.from_xyz(x + GALAXY_SEPARATION / 2, y, z),
            Mesh3d(sphere_mesh),
            MeshMaterial3d(galaxy2_mat),
        )

    # Camera setup
    camera_distance = 150.0
    camera_height = 80.0
    target = Vec3.ZERO
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, camera_height, camera_distance).looking_at(
            target, Vec3.Y
        ),
        OrbitCamera(
            distance=math.sqrt(camera_height**2 + camera_distance**2),
            yaw=0.0,
            pitch=-0.5,
            target=target,
        ),
    )

    # Lighting (shadows disabled for performance with 96k particles)
    # Dim directional light + ambient to make emissive glow stand out
    commands.spawn(
        DirectionalLight(illuminance=2000.0, shadows_enabled=False),
        Transform.from_xyz(50.0, 100.0, 50.0).looking_at(Vec3.ZERO, Vec3.Y),
    )

    # Add ambient light for better visibility
    commands.insert_resource(GlobalAmbientLight(brightness=300.0, color=Color.srgb(0.15, 0.15, 0.2)))



@numba.jit(nopython=True)
def compute_octant(
    px: float, py: float, pz: float, center_x: float, center_y: float, center_z: float
) -> int:
    """Compute which octant (0-7) a point belongs to relative to center."""
    octant = 0
    if px >= center_x:
        octant |= 1
    if py >= center_y:
        octant |= 2
    if pz >= center_z:
        octant |= 4
    return octant


@numba.jit(nopython=True)
def octant_center(
    parent_cx: float,
    parent_cy: float,
    parent_cz: float,
    parent_size: float,
    octant: int,
) -> tuple[float, float, float]:
    """Compute center of octant child."""
    half_size = parent_size / 4.0  # Quarter of parent size

    cx = parent_cx + half_size if (octant & 1) else parent_cx - half_size
    cy = parent_cy + half_size if (octant & 2) else parent_cy - half_size
    cz = parent_cz + half_size if (octant & 4) else parent_cz - half_size

    return cx, cy, cz


@numba.jit(nopython=True)
def build_octree(
    pos_x: np.ndarray,
    pos_y: np.ndarray,
    pos_z: np.ndarray,
    masses: np.ndarray,
    node_center_x: np.ndarray,
    node_center_y: np.ndarray,
    node_center_z: np.ndarray,
    node_size: np.ndarray,
    node_mass: np.ndarray,
    node_com_x: np.ndarray,
    node_com_y: np.ndarray,
    node_com_z: np.ndarray,
    node_particle_count: np.ndarray,
    node_is_leaf: np.ndarray,
    node_children: np.ndarray,
    particle_node_ids: np.ndarray,
) -> int:
    """Build octree from particles using recursive subdivision.

    Returns total number of nodes created.
    """
    n_particles = len(pos_x)
    node_count = 1  # Start with root node

    # Initialize root node
    node_center_x[0] = 0.0
    node_center_y[0] = 0.0
    node_center_z[0] = 0.0
    node_size[0] = TREE_ROOT_SIZE
    node_mass[0] = 0.0
    node_com_x[0] = 0.0
    node_com_y[0] = 0.0
    node_com_z[0] = 0.0
    node_particle_count[0] = 0
    node_is_leaf[0] = True
    for k in range(8):
        node_children[0, k] = -1

    # Insert particles into tree
    for i in range(n_particles):
        px, py, pz = pos_x[i], pos_y[i], pos_z[i]
        mass = masses[i]

        # Traverse from root to find insertion point
        node_id = 0
        depth = 0

        while depth < MAX_TREE_DEPTH:
            # Update node COM and mass
            total_mass = node_mass[node_id] + mass
            if total_mass > 1e-10:
                node_com_x[node_id] = (
                    node_com_x[node_id] * node_mass[node_id] + px * mass
                ) / total_mass
                node_com_y[node_id] = (
                    node_com_y[node_id] * node_mass[node_id] + py * mass
                ) / total_mass
                node_com_z[node_id] = (
                    node_com_z[node_id] * node_mass[node_id] + pz * mass
                ) / total_mass
            node_mass[node_id] = total_mass
            node_particle_count[node_id] += 1

            # If leaf node with space, insert here
            if node_is_leaf[node_id]:
                if node_particle_count[node_id] <= MAX_PARTICLES_PER_LEAF:
                    particle_node_ids[i] = node_id
                    break
                # Subdivide this leaf
                node_is_leaf[node_id] = False

            # Determine which child octant
            octant = compute_octant(
                px,
                py,
                pz,
                node_center_x[node_id],
                node_center_y[node_id],
                node_center_z[node_id],
            )

            # Create child if doesn't exist
            if node_children[node_id, octant] == -1:
                child_id = node_count
                node_count += 1

                if node_count >= len(node_center_x):
                    # Tree full - stop insertion
                    return node_count

                # Initialize child
                cx, cy, cz = octant_center(
                    node_center_x[node_id],
                    node_center_y[node_id],
                    node_center_z[node_id],
                    node_size[node_id],
                    octant,
                )
                child_size = node_size[node_id] / 2.0

                node_center_x[child_id] = cx
                node_center_y[child_id] = cy
                node_center_z[child_id] = cz
                node_size[child_id] = child_size
                node_mass[child_id] = 0.0
                node_com_x[child_id] = 0.0
                node_com_y[child_id] = 0.0
                node_com_z[child_id] = 0.0
                node_particle_count[child_id] = 0
                node_is_leaf[child_id] = True
                for k in range(8):
                    node_children[child_id, k] = -1

                node_children[node_id, octant] = child_id

            # Move to child
            node_id = node_children[node_id, octant]
            depth += 1

        if depth >= MAX_TREE_DEPTH:
            # Max depth reached - assign to current node
            particle_node_ids[i] = node_id

    return node_count


@numba.jit(nopython=True)
def compute_force_tree_walk(
    px: float,
    py: float,
    pz: float,
    node_id: int,
    node_center_x: np.ndarray,
    node_center_y: np.ndarray,
    node_center_z: np.ndarray,
    node_size: np.ndarray,
    node_mass: np.ndarray,
    node_com_x: np.ndarray,
    node_com_y: np.ndarray,
    node_com_z: np.ndarray,
    node_is_leaf: np.ndarray,
    node_children: np.ndarray,
) -> tuple[float, float, float]:
    """Compute gravitational force on particle at (px, py, pz) via tree walk.

    Returns (fx, fy, fz).
    """
    fx, fy, fz = 0.0, 0.0, 0.0

    # Stack for iterative tree traversal (avoid recursion in Numba)
    stack = np.zeros(128, dtype=np.int32)
    stack[0] = node_id
    stack_size = 1

    while stack_size > 0:
        stack_size -= 1
        current = stack[stack_size]

        if current == -1 or node_mass[current] < 1e-10:
            continue

        # Compute distance to node COM
        dx = node_com_x[current] - px
        dy = node_com_y[current] - py
        dz = node_com_z[current] - pz
        dist_sq = dx * dx + dy * dy + dz * dz + SOFTENING * SOFTENING

        # Barnes-Hut criterion: s / d < θ
        node_s = node_size[current]
        if node_s * node_s / dist_sq < THETA * THETA or node_is_leaf[current]:
            # Treat as single point mass
            dist = math.sqrt(dist_sq)
            force_mag = G * node_mass[current] / (dist_sq * dist)
            fx += force_mag * dx
            fy += force_mag * dy
            fz += force_mag * dz
        else:
            # Recurse into children (push onto stack)
            for k in range(8):
                child = node_children[current, k]
                if child != -1 and stack_size < len(stack):
                    stack[stack_size] = child
                    stack_size += 1

    return fx, fy, fz


@numba.jit(nopython=True, parallel=True)
def compute_forces(
    pos_x: np.ndarray,
    pos_y: np.ndarray,
    pos_z: np.ndarray,
    accel_x: np.ndarray,
    accel_y: np.ndarray,
    accel_z: np.ndarray,
    masses: np.ndarray,
    node_center_x: np.ndarray,
    node_center_y: np.ndarray,
    node_center_z: np.ndarray,
    node_size: np.ndarray,
    node_mass: np.ndarray,
    node_com_x: np.ndarray,
    node_com_y: np.ndarray,
    node_com_z: np.ndarray,
    node_is_leaf: np.ndarray,
    node_children: np.ndarray,
) -> None:
    """Compute gravitational forces on all particles using Barnes-Hut."""
    n = len(pos_x)

    for i in numba.prange(n):
        fx, fy, fz = compute_force_tree_walk(
            pos_x[i],
            pos_y[i],
            pos_z[i],
            0,  # Start from root
            node_center_x,
            node_center_y,
            node_center_z,
            node_size,
            node_mass,
            node_com_x,
            node_com_y,
            node_com_z,
            node_is_leaf,
            node_children,
        )

        # a = F / m
        mass = masses[i]
        if mass > 1e-10:
            accel_x[i] = fx / mass
            accel_y[i] = fy / mass
            accel_z[i] = fz / mass
        else:
            accel_x[i] = 0.0
            accel_y[i] = 0.0
            accel_z[i] = 0.0


@numba.jit(nopython=True)
def leapfrog_integrate(
    pos_x: np.ndarray,
    pos_y: np.ndarray,
    pos_z: np.ndarray,
    vel_x: np.ndarray,
    vel_y: np.ndarray,
    vel_z: np.ndarray,
    accel_x: np.ndarray,
    accel_y: np.ndarray,
    accel_z: np.ndarray,
    dt: float,
) -> None:
    """Leapfrog integration: kick-drift-kick for symplectic time stepping."""
    n = len(pos_x)

    for i in range(n):
        # Half-step velocity kick
        vx = vel_x[i] + accel_x[i] * dt * 0.5
        vy = vel_y[i] + accel_y[i] * dt * 0.5
        vz = vel_z[i] + accel_z[i] * dt * 0.5

        vel_x[i] = vx
        vel_y[i] = vy
        vel_z[i] = vz

        # Full-step position drift
        pos_x[i] = pos_x[i] + vx * dt
        pos_y[i] = pos_y[i] + vy * dt
        pos_z[i] = pos_z[i] + vz * dt


@numba.jit(nopython=True)
def velocity_kick(
    vel_x: np.ndarray,
    vel_y: np.ndarray,
    vel_z: np.ndarray,
    accel_x: np.ndarray,
    accel_y: np.ndarray,
    accel_z: np.ndarray,
    dt: float,
) -> None:
    """Second half of leapfrog: final velocity kick."""
    n = len(vel_x)

    for i in range(n):
        vel_x[i] += accel_x[i] * dt * 0.5
        vel_y[i] += accel_y[i] * dt * 0.5
        vel_z[i] += accel_z[i] * dt * 0.5


def physics_system(
    query: Query[tuple[Mut[Particle], Mut[Transform]], With[SimMarker]],
    sim_state: ResMut[SimulationState],
) -> None:
    """Main N-body physics system using Barnes-Hut algorithm."""
    # Extract all particle data to numpy arrays via Query iteration
    pos_x_list: list[float] = []
    pos_y_list: list[float] = []
    pos_z_list: list[float] = []
    vel_x_list: list[float] = []
    vel_y_list: list[float] = []
    vel_z_list: list[float] = []
    mass_list: list[float] = []

    items = list(query)
    for particle, _transform in items:
        pos_x_list.append(particle.pos.x)
        pos_y_list.append(particle.pos.y)
        pos_z_list.append(particle.pos.z)
        vel_x_list.append(particle.vel.x)
        vel_y_list.append(particle.vel.y)
        vel_z_list.append(particle.vel.z)
        mass_list.append(particle.mass)

    if not pos_x_list:
        return

    pos_x = np.array(pos_x_list, dtype=np.float32)
    pos_y = np.array(pos_y_list, dtype=np.float32)
    pos_z = np.array(pos_z_list, dtype=np.float32)
    vel_x = np.array(vel_x_list, dtype=np.float32)
    vel_y = np.array(vel_y_list, dtype=np.float32)
    vel_z = np.array(vel_z_list, dtype=np.float32)
    masses = np.array(mass_list, dtype=np.float32)

    n = len(pos_x)
    accel_x = np.zeros(n, dtype=np.float32)
    accel_y = np.zeros(n, dtype=np.float32)
    accel_z = np.zeros(n, dtype=np.float32)

    # Reset tree
    sim_state.node_count = 0
    sim_state.node_mass[:] = 0.0

    # Build octree
    node_count = build_octree(
        pos_x,
        pos_y,
        pos_z,
        masses,
        sim_state.node_center_x,
        sim_state.node_center_y,
        sim_state.node_center_z,
        sim_state.node_size,
        sim_state.node_mass,
        sim_state.node_com_x,
        sim_state.node_com_y,
        sim_state.node_com_z,
        sim_state.node_particle_count,
        sim_state.node_is_leaf,
        sim_state.node_children,
        sim_state.particle_node_ids,
    )
    sim_state.node_count = node_count

    # Compute forces using Barnes-Hut
    compute_forces(
        pos_x,
        pos_y,
        pos_z,
        accel_x,
        accel_y,
        accel_z,
        masses,
        sim_state.node_center_x,
        sim_state.node_center_y,
        sim_state.node_center_z,
        sim_state.node_size,
        sim_state.node_mass,
        sim_state.node_com_x,
        sim_state.node_com_y,
        sim_state.node_com_z,
        sim_state.node_is_leaf,
        sim_state.node_children,
    )

    # Leapfrog integration on numpy arrays
    leapfrog_integrate(pos_x, pos_y, pos_z, vel_x, vel_y, vel_z, accel_x, accel_y, accel_z, DT)

    # Final velocity kick
    velocity_kick(vel_x, vel_y, vel_z, accel_x, accel_y, accel_z, DT)

    # Write back to ECS (use stored list - query iterator is exhausted)
    for i, (particle, transform) in enumerate(items):
        particle.pos = Vec3(float(pos_x[i]), float(pos_y[i]), float(pos_z[i]))
        particle.vel = Vec3(float(vel_x[i]), float(vel_y[i]), float(vel_z[i]))
        particle.accel = Vec3(float(accel_x[i]), float(accel_y[i]), float(accel_z[i]))
        transform.translation = particle.pos

    # Update statistics
    sim_state.frame_count += 1
    if sim_state.frame_count % 60 == 0:
        # Compute energies
        kinetic = 0.5 * np.sum(masses * (vel_x**2 + vel_y**2 + vel_z**2))
        sim_state.kinetic_energy = float(kinetic)
        print(f"Frame {sim_state.frame_count}: KE={kinetic:.1f}, Nodes={node_count}")


@entrypoint
def main(app: App) -> App:
    """Configure and run the Barnes-Hut N-body simulation."""
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(OrbitCameraPlugin())
        .insert_resource(SimulationState())
        .add_systems(Startup, setup_simulation)
        .add_systems(Update, physics_system)
    )


if __name__ == "__main__":
    print(f"Barnes-Hut N-body: {N_PARTICLES:,} particles, drag mouse to rotate camera.")
    main().run()
