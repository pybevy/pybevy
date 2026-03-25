"""
City simulation with streets, buildings, and 150,000 randomly walking people.
Uses ViewColumn + Numba JIT for high-performance entity updates.

Performance:
- View + Numba (parallel): ~5-10ms per frame for 150k entities
- Traditional Query: ~500-1000ms per frame
- Speedup: ~100x faster than Query iteration
"""

import math
import random

try:
    import numba  # type: ignore[import-untyped]
except ImportError:
    print("ERROR: Numba is required for this example.")
    print("Install with: poetry install --extras numba")
    print("Or: pip install numba")
    exit(1)

from dataclasses import dataclass

import numpy as np

from pybevy.prelude import *

# Constants
NUM_PEOPLE = 25000


# Global camera path state
class CameraPathState:
    """Global state for cinematic camera path."""

    def __init__(self):
        self.waypoints = []
        self.current_waypoint = 0
        self.initial_wait = 15.0  # seconds to wait before starting movement
        self.transition_duration = 5.0  # seconds to move between waypoints
        self.hold_duration = 2.0  # seconds to hold at each waypoint
        self.transition_start_time = 0.0
        self.has_started = False  # Track if initial wait has completed


# Global instance
CAMERA_PATH = CameraPathState()


@component
class Walker(Component):
    """Component marking an entity as a walking person."""



@component
class FlyingCamera(Component):
    """Component marking the camera as flying around the city."""



@component
class WarningLight(Component):
    """Component marking a pulsating warning light with individual phase offset."""

    def __init__(self, phase_offset: float = 0.0):
        self.phase_offset = phase_offset


@dataclass
@resource
class CityLayout(Resource):
    """Resource storing the city's street layout for pathfinding."""

    street_width: float = 2.0
    block_size: float = 10.0
    num_blocks_x: int = 15
    num_blocks_z: int = 15


def setup_city(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    layout = CityLayout()

    # Materials
    ground_material = materials.add(Color.srgb(0.3, 0.4, 0.3))
    street_material = materials.add(Color.srgb(0.2, 0.2, 0.2))
    building_materials = [
        materials.add(Color.srgb(0.6, 0.6, 0.7)),
        materials.add(Color.srgb(0.7, 0.6, 0.6)),
        materials.add(Color.srgb(0.6, 0.7, 0.6)),
        materials.add(Color.srgb(0.5, 0.5, 0.6)),
    ]

    # Ground plane
    city_size = (
        layout.num_blocks_x * (layout.block_size + layout.street_width)
    ) + layout.street_width
    ground_plane = Plane3d()
    commands.spawn(
        Mesh3d(meshes.add(ground_plane.mesh().size(city_size, city_size).build())),
        MeshMaterial3d(ground_material),
        Transform.from_xyz(city_size / 2.0, -0.05, city_size / 2.0),
    )

    # Create streets (horizontal and vertical)
    street_mesh = meshes.add(Cuboid(1.0, 0.1, 1.0))

    # Vertical streets
    for x_idx in range(layout.num_blocks_x + 1):
        x_pos = x_idx * (layout.block_size + layout.street_width)
        street_length = city_size
        commands.spawn(
            Mesh3d(street_mesh),
            MeshMaterial3d(street_material),
            Transform(
                Vec3(x_pos + layout.street_width / 2.0, 0.0, city_size / 2.0),
                Quat.IDENTITY,
                Vec3(layout.street_width, 1.0, street_length),
            ),
        )

    # Horizontal streets
    for z_idx in range(layout.num_blocks_z + 1):
        z_pos = z_idx * (layout.block_size + layout.street_width)
        street_length = city_size
        commands.spawn(
            Mesh3d(street_mesh),
            MeshMaterial3d(street_material),
            Transform(
                Vec3(city_size / 2.0, 0.0, z_pos + layout.street_width / 2.0),
                Quat.IDENTITY,
                Vec3(street_length, 1.0, layout.street_width),
            ),
        )

    # Create buildings in blocks
    building_mesh = meshes.add(Cuboid.from_length(1.0))

    # Red warning light for tall buildings
    warning_light_mesh = meshes.add(Sphere(0.2))
    warning_light_material = materials.add(Color.srgb(1.0, 0.0, 0.0))

    for bx in range(layout.num_blocks_x):
        for bz in range(layout.num_blocks_z):
            # Random number of buildings per block (1-4)
            num_buildings = random.randint(1, 4)

            block_x = (
                bx * (layout.block_size + layout.street_width) + layout.street_width
            )
            block_z = (
                bz * (layout.block_size + layout.street_width) + layout.street_width
            )

            for _ in range(num_buildings):
                # Random building dimensions
                width = random.uniform(2.0, 5.0)
                depth = random.uniform(2.0, 5.0)
                height = random.uniform(5.0, 25.0)

                # Random position within block (with margins)
                margin = 1.0
                x: float = block_x + random.uniform(margin, layout.block_size - width - margin)
                z: float = block_z + random.uniform(margin, layout.block_size - depth - margin)

                # Spawn building with static physics collider
                commands.spawn(
                    # RigidBody.static(),
                    # Collider.cuboid(width, height, depth),
                    Mesh3d(building_mesh),
                    MeshMaterial3d(random.choice(building_materials)),
                    Transform(
                        Vec3(x + width / 2.0, height / 2.0, z + depth / 2.0),
                        Quat.IDENTITY,
                        Vec3(width, height, depth),
                    ),
                )

                # Add red warning lights to top 10% of tallest buildings
                if height >= 22.5:  # Top 10% (heights 22.5-25.0 out of 5.0-25.0)
                    building_top = height
                    center_x = x + width / 2.0
                    center_z = z + depth / 2.0

                    # Place red lights at the four corners of the building top
                    corner_offset_x = width / 2.0 * 0.9
                    corner_offset_z = depth / 2.0 * 0.9

                    for corner_x in [-corner_offset_x, corner_offset_x]:
                        for corner_z in [-corner_offset_z, corner_offset_z]:
                            # Random phase offset for non-uniform pulsation
                            phase_offset = random.uniform(0.0, 2.0 * math.pi)

                            # Spawn warning light mesh
                            commands.spawn(
                                Mesh3d(warning_light_mesh),
                                MeshMaterial3d(warning_light_material),
                                Transform.from_xyz(
                                    center_x + corner_x,
                                    building_top + 0.3,
                                    center_z + corner_z,
                                ),
                            )

                            # Spawn point light
                            commands.spawn(
                                PointLight(
                                    intensity=3_000.0,
                                    range=8.0,
                                    radius=0.1,
                                    color=Color.srgb(1.0, 0.0, 0.0),
                                    shadows_enabled=False,
                                ),
                                Transform.from_xyz(
                                    center_x + corner_x,
                                    building_top + 0.3,
                                    center_z + corner_z,
                                ),
                                WarningLight(phase_offset),
                            )

    # Create street lights along vertical streets
    light_spacing = 8.0  # Distance between lights
    light_height = 2.5
    light_offset = layout.street_width * 0.6  # Offset from street center

    # Light pole mesh
    pole_mesh = meshes.add(Cuboid(0.1, light_height, 0.1))
    pole_material = materials.add(Color.srgb(0.3, 0.3, 0.3))

    # Light bulb mesh (sphere at top of pole)
    bulb_mesh = meshes.add(Sphere(0.15))
    bulb_material = materials.add(Color.srgb(1.0, 0.95, 0.8))

    for x in range(layout.num_blocks_x + 1):
        x_pos = (
            x * (layout.block_size + layout.street_width) + layout.street_width / 2.0
        )
        num_lights = int(city_size / light_spacing)

        for i in range(num_lights):
            z_pos = i * light_spacing

            # Lights on both sides of the street
            for side in [-1, 1]:
                light_x = x_pos + (side * light_offset)

                # Spawn light pole
                commands.spawn(
                    Mesh3d(pole_mesh),
                    MeshMaterial3d(pole_material),
                    Transform.from_xyz(light_x, light_height / 2.0, z_pos),
                )

                # Spawn light bulb at top of pole
                commands.spawn(
                    Mesh3d(bulb_mesh),
                    MeshMaterial3d(bulb_material),
                    Transform.from_xyz(light_x, light_height, z_pos),
                )

                # Spawn point light at top of pole
                commands.spawn(
                    PointLight(
                        intensity=2_000.0,
                        range=12.0,
                        radius=0.2,
                        color=Color.srgb(1.0, 0.9, 0.7),
                        shadows_enabled=False,
                    ),
                    Transform.from_xyz(light_x, light_height, z_pos),
                )

    # Create street lights along horizontal streets
    for z in range(layout.num_blocks_z + 1):
        z_pos = (
            z * (layout.block_size + layout.street_width) + layout.street_width / 2.0
        )
        num_lights = int(city_size / light_spacing)

        for i in range(num_lights):
            x_pos = i * light_spacing

            # Lights on both sides of the street
            for side in [-1, 1]:
                light_z = z_pos + (side * light_offset)

                # Spawn light pole
                commands.spawn(
                    Mesh3d(pole_mesh),
                    MeshMaterial3d(pole_material),
                    Transform.from_xyz(x_pos, light_height / 2.0, light_z),
                )

                # Spawn light bulb at top of pole
                commands.spawn(
                    Mesh3d(bulb_mesh),
                    MeshMaterial3d(bulb_material),
                    Transform.from_xyz(x_pos, light_height, light_z),
                )

                # Spawn point light at top of pole
                commands.spawn(
                    PointLight(
                        intensity=5_000.0,
                        range=12.0,
                        radius=0.2,
                        color=Color.srgb(1.0, 0.9, 0.7),
                        shadows_enabled=False,
                    ),
                    Transform.from_xyz(x_pos, light_height, light_z),
                )


def spawn_people(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    layout = CityLayout()
    city_size = (
        layout.num_blocks_x * (layout.block_size + layout.street_width)
    ) + layout.street_width

    person_mesh = meshes.add(Cuboid(0.15, 0.6, 0.16))
    person_materials = [
        materials.add(Color.srgb(0.8, 0.2, 0.2)),
        materials.add(Color.srgb(0.2, 0.8, 0.2)),
        materials.add(Color.srgb(0.2, 0.2, 0.8)),
        materials.add(Color.srgb(0.8, 0.8, 0.2)),
        materials.add(Color.srgb(0.8, 0.2, 0.8)),
        materials.add(Color.srgb(0.2, 0.8, 0.8)),
    ]

    for _ in range(NUM_PEOPLE):
        # Spawn on streets - either on horizontal or vertical streets
        if random.random() < 0.5:
            # Vertical street
            street_idx = random.randint(0, layout.num_blocks_x)
            x = (
                street_idx * (layout.block_size + layout.street_width)
                + layout.street_width / 2.0
            )
            x += random.uniform(-layout.street_width / 3.0, layout.street_width / 3.0)
            z = random.uniform(0, city_size)
        else:
            # Horizontal street
            street_idx = random.randint(0, layout.num_blocks_z)
            z = (
                street_idx * (layout.block_size + layout.street_width)
                + layout.street_width / 2.0
            )
            z += random.uniform(-layout.street_width / 3.0, layout.street_width / 3.0)
            x = random.uniform(0, city_size)

        # Random initial direction
        angle = random.uniform(0, 2 * math.pi)

        # Place person center at half mesh height so feet touch ground
        # Person mesh is Cuboid(0.15, 0.6, 0.16) → height 0.6, center at y=0.3
        person_y = 0.3

        # Spawn person with kinematic rigidbody and collider
        commands.spawn(
            # RigidBody.kinematic(),
            # Collider.cuboid(0.4, 1.5, 0.4),
            Mesh3d(person_mesh),
            MeshMaterial3d(random.choice(person_materials)),
            Transform.from_xyz(x, person_y, z).with_rotation(Quat.from_rotation_y(angle)),
            Walker(),
        )


def setup_scene(
    commands: Commands,
) -> None:
    layout = CityLayout()
    city_size = (
        layout.num_blocks_x * (layout.block_size + layout.street_width)
    ) + layout.street_width
    center = city_size / 2.0

    # Directional light (sun)
    """
    commands.spawn(
        DirectionalLight(illuminance=10000.0, shadows_enabled=True),
        Transform.from_xyz(center, 50.0, center).looking_at(
            Vec3(center, 0, center), Vec3.Y
        ),
    )

    commands.spawn(
        PointLight(shadows_enabled=True, intensity=2_000_000, range=1000),
        Transform.from_xyz(center, 50.0, center),
    )
    """

    # Create cinematic camera waypoints
    camera_path = CAMERA_PATH

    # Waypoint format: (camera_position, look_at_target)
    # 0. Start position - even further away, slowly moving forward
    camera_path.waypoints.append(
        (
            np.array([center, 150.0, city_size + 120.0]),  # Very far away, high
            np.array([center, 0.0, center]),  # Look at city center
        )
    )

    # 1. Move closer - aerial view (end of slow forward movement)
    camera_path.waypoints.append(
        (
            np.array([center, 150.0, city_size + 80.0]),  # Far away, high
            np.array([center, 0.0, center]),  # Look at city center
        )
    )

    # 2. Street level - move left along the street while turning right
    # Position on a vertical street, looking down it initially
    street_x = layout.street_width * 2.5 + layout.block_size * 2
    start_z = center - 20.0
    camera_path.waypoints.append(
        (
            np.array([street_x - 3.0, 2.5, start_z]),  # On the side of the street
            np.array(
                [street_x + 5.0, 2.0, start_z + 25.0]
            ),  # Look right down the street
        )
    )

    # 3. Rise up from street, then move to corner with descending view
    # First go up from previous position
    corner_x = layout.street_width + layout.block_size * 1.0
    corner_z = layout.street_width + layout.block_size * 1.0
    camera_path.waypoints.append(
        (
            np.array([corner_x - 12.0, 35.0, corner_z - 12.0]),  # High corner position
            np.array(
                [corner_x + 30.0, 8.0, corner_z + 30.0]
            ),  # Look down along diagonal
        )
    )

    # 4. Mid-level shot of a building
    building_x = layout.street_width + layout.block_size / 2.0
    building_z = layout.street_width + layout.block_size / 2.0
    camera_path.waypoints.append(
        (
            np.array([building_x - 15.0, 20.0, building_z + 15.0]),
            np.array([building_x, 10.0, building_z]),
        )
    )

    # 5. Another alley angle - elevated view looking down
    alley_center_x = layout.street_width + layout.block_size * 4.5
    alley_center_z = layout.street_width * 5 + layout.block_size * 4.5
    camera_path.waypoints.append(
        (
            np.array([alley_center_x, 12.0, alley_center_z]),
            np.array([alley_center_x - 15.0, 0.0, alley_center_z + 10.0]),
        )
    )

    # 6. Forward movement over city - continue trajectory
    mid_flight_x = center + 20.0
    mid_flight_z = center + 20.0
    camera_path.waypoints.append(
        (
            np.array([mid_flight_x, 45.0, mid_flight_z]),
            np.array([mid_flight_x + 30.0, 20.0, mid_flight_z + 30.0]),  # Look forward
        )
    )

    # 7. Rotate 180 degrees and descend - looking back over the path
    camera_path.waypoints.append(
        (
            np.array(
                [mid_flight_x + 35.0, 25.0, mid_flight_z + 35.0]
            ),  # Continue forward but descend
            np.array(
                [mid_flight_x - 10.0, 5.0, mid_flight_z - 10.0]
            ),  # Look back (180 rotation) and down
        )
    )

    # 8. Return to aerial view (loops back to start)
    camera_path.waypoints.append(
        (np.array([center, 150.0, city_size + 120.0]), np.array([center, 0.0, center]))
    )

    # Camera starts at first waypoint
    start_pos, start_target = camera_path.waypoints[0]
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(start_pos[0], start_pos[1], start_pos[2]).looking_at(
            Vec3(start_target[0], start_target[1], start_target[2]), Vec3.Y
        ),
        FlyingCamera(),
    )


@numba.jit(nopython=True)  # type: ignore[misc]
def walking_kernel(
    translation,
    rotation,
    delta_time: float,
    city_size: float,
    speed: float,
    turn_chance: float,
    margin: float,
) -> None:
    """Update walker positions with random movement (PARALLEL).

    Args:
        translation: Vec3ViewColumn for Transform.translation
        rotation: QuatViewColumn for Transform.rotation
        delta_time: Frame delta time in seconds
        city_size: Total city size for boundary checks
        speed: Movement speed in units/second
        turn_chance: Probability per frame to turn randomly
        margin: Distance from edge to trigger bounce
    """
    n = len(translation.x)

    for i in numba.prange(n):  # Parallel loop across CPU cores
        # Read current rotation quaternion
        qx = rotation.x[i]
        qy = rotation.y[i]
        qz = rotation.z[i]
        qw = rotation.w[i]

        # Calculate forward direction from quaternion
        # Forward in Bevy is -Z axis rotated by quaternion
        # R * (0, 0, -1) = (-R[0][2], -R[1][2], -R[2][2])
        forward_x = -2.0 * (qx * qz + qw * qy)
        forward_z = 2.0 * (qx * qx + qy * qy) - 1.0

        # Move forward (only X-Z plane)
        delta = speed * delta_time
        new_x = translation.x[i] + forward_x * delta
        new_z = translation.z[i] + forward_z * delta

        # Random turn
        if random.random() < turn_chance:
            # Random turn angle
            turn_angle = random.uniform(-math.pi, math.pi)
            half_angle = turn_angle / 2.0
            sin_half = math.sin(half_angle)
            cos_half = math.cos(half_angle)

            # Quaternion multiplication: current * turn_quat
            # turn_quat for Y-axis: (0, sin(θ/2), 0, cos(θ/2))
            new_qx = qw * 0.0 + qx * cos_half + qy * 0.0 - qz * sin_half
            new_qy = qw * sin_half - qx * 0.0 + qy * cos_half + qz * 0.0
            new_qz = qw * 0.0 + qx * sin_half - qy * 0.0 + qz * cos_half
            new_qw = qw * cos_half - qx * 0.0 - qy * sin_half - qz * 0.0

            rotation.x[i] = new_qx
            rotation.y[i] = new_qy
            rotation.z[i] = new_qz
            rotation.w[i] = new_qw

        # Boundary checking and bounce
        bounce = False
        if new_x < margin or new_x > city_size - margin:
            bounce = True
        if new_z < margin or new_z > city_size - margin:
            bounce = True

        if bounce:
            # 180-degree rotation around Y: q * (0, 1, 0, 0)
            # Hamilton product: w=-y1, x=-z1, y=w1, z=x1
            cur_x = rotation.x[i]
            cur_y = rotation.y[i]
            cur_z = rotation.z[i]
            cur_w = rotation.w[i]

            rotation.x[i] = -cur_z
            rotation.y[i] = cur_w
            rotation.z[i] = cur_x
            rotation.w[i] = -cur_y

            # Clamp position
            new_x = max(margin, min(city_size - margin, new_x))
            new_z = max(margin, min(city_size - margin, new_z))

        # Write position
        translation.x[i] = new_x
        translation.z[i] = new_z


def walking_system(
    view: View[Mut[Transform], With[Walker]],
    time: Res[Time],
) -> None:
    """Move people around randomly using ViewColumn + Numba JIT."""
    layout = CityLayout()
    city_size = (
        layout.num_blocks_x * (layout.block_size + layout.street_width)
    ) + layout.street_width

    speed = 2.0
    turn_chance = 0.05
    margin = 1.0

    for batch in view.iter_batches():
        transform = batch.column_mut(Transform)

        walking_kernel(
            transform.translation,
            transform.rotation,
            time.delta_secs(),
            city_size,
            speed,
            turn_chance,
            margin,
        )


def warning_light_pulsate_system(
    query: Query[tuple[Mut[PointLight], WarningLight]],
    time: Res[Time],
) -> None:
    """Make warning lights pulsate."""
    frequency = 0.5
    min_intensity = 10_000.0
    max_intensity = 50_000.0
    base_time = time.elapsed_secs() * frequency * 2.0 * math.pi

    for light, warning in query:
        t = base_time + warning.phase_offset
        pulse = (math.sin(t) + 1.0) / 2.0
        light.intensity = min_intensity + (max_intensity - min_intensity) * pulse


@numba.jit(nopython=True)  # type: ignore[misc]
def camera_look_at_kernel(
    translation,
    rotation,
    camera_x: float,
    camera_y: float,
    camera_z: float,
    target_x: float,
    target_y: float,
    target_z: float,
) -> None:
    """Update camera transform to look at target (non-parallel for single camera)."""
    # Set camera position
    translation.x[0] = camera_x
    translation.y[0] = camera_y
    translation.z[0] = camera_z

    # Calculate look_at direction
    forward_x = target_x - camera_x
    forward_y = target_y - camera_y
    forward_z = target_z - camera_z

    forward_norm = math.sqrt(
        forward_x * forward_x + forward_y * forward_y + forward_z * forward_z
    )

    if forward_norm > 0.0001:
        # Normalize forward
        forward_x /= forward_norm
        forward_y /= forward_norm
        forward_z /= forward_norm

        # World up vector
        world_up_x = 0.0
        world_up_y = 1.0
        world_up_z = 0.0

        # Right = forward x world_up
        right_x = forward_y * world_up_z - forward_z * world_up_y
        right_y = forward_z * world_up_x - forward_x * world_up_z
        right_z = forward_x * world_up_y - forward_y * world_up_x

        right_norm = math.sqrt(right_x * right_x + right_y * right_y + right_z * right_z)

        if right_norm > 0.0001:
            # Normalize right
            right_x /= right_norm
            right_y /= right_norm
            right_z /= right_norm

            # Up = right x forward
            up_x = right_y * forward_z - right_z * forward_y
            up_y = right_z * forward_x - right_x * forward_z
            up_z = right_x * forward_y - right_y * forward_x

            up_norm = math.sqrt(up_x * up_x + up_y * up_y + up_z * up_z)
            up_x /= up_norm
            up_y /= up_norm
            up_z /= up_norm

            # Build rotation matrix: [right, up, -forward]
            m00, m01, m02 = right_x, up_x, -forward_x
            m10, m11, m12 = right_y, up_y, -forward_y
            m20, m21, m22 = right_z, up_z, -forward_z

            # Convert to quaternion
            trace = m00 + m11 + m22

            if trace > 0.0:
                s = 0.5 / math.sqrt(trace + 1.0)
                qw = 0.25 / s
                qx = (m21 - m12) * s
                qy = (m02 - m20) * s
                qz = (m10 - m01) * s
            elif m00 > m11 and m00 > m22:
                s = 2.0 * math.sqrt(1.0 + m00 - m11 - m22)
                qw = (m21 - m12) / s
                qx = 0.25 * s
                qy = (m01 + m10) / s
                qz = (m02 + m20) / s
            elif m11 > m22:
                s = 2.0 * math.sqrt(1.0 + m11 - m00 - m22)
                qw = (m02 - m20) / s
                qx = (m01 + m10) / s
                qy = 0.25 * s
                qz = (m12 + m21) / s
            else:
                s = 2.0 * math.sqrt(1.0 + m22 - m00 - m11)
                qw = (m10 - m01) / s
                qx = (m02 + m20) / s
                qy = (m12 + m21) / s
                qz = 0.25 * s

            rotation.x[0] = qx
            rotation.y[0] = qy
            rotation.z[0] = qz
            rotation.w[0] = qw


def camera_flight_system(
    view: View[Mut[Transform], With[FlyingCamera]],
    time: Res[Time],
) -> None:
    """Cinematic camera system with smooth transitions between waypoints."""
    camera_path = CAMERA_PATH

    current_time = time.elapsed_secs()

    # Initialize transition start time on first frame
    if camera_path.transition_start_time == 0.0:
        camera_path.transition_start_time = current_time

    # Handle initial wait period
    if not camera_path.has_started:
        elapsed_since_start = current_time - camera_path.transition_start_time
        if elapsed_since_start < camera_path.initial_wait:
            # Still waiting - stay at first waypoint
            start_pos, start_target = camera_path.waypoints[0]

            for batch in view.iter_batches():
                transform = batch.column_mut(Transform)
                camera_look_at_kernel(
                    transform.translation,
                    transform.rotation,
                    start_pos[0],
                    start_pos[1],
                    start_pos[2],
                    start_target[0],
                    start_target[1],
                    start_target[2],
                )
            return
        # Initial wait complete
        camera_path.has_started = True
        camera_path.transition_start_time = current_time

    # Time since current transition started
    elapsed = current_time - camera_path.transition_start_time
    total_duration = camera_path.transition_duration + camera_path.hold_duration

    # Move to next waypoint if needed
    if elapsed >= total_duration:
        camera_path.current_waypoint = (camera_path.current_waypoint + 1) % len(
            camera_path.waypoints
        )
        camera_path.transition_start_time = current_time
        elapsed = 0.0

    # Get current and next waypoint
    current_idx = camera_path.current_waypoint
    next_idx = (current_idx + 1) % len(camera_path.waypoints)

    current_pos, current_target = camera_path.waypoints[current_idx]
    next_pos, next_target = camera_path.waypoints[next_idx]

    # Calculate interpolation with smoothstep
    if elapsed < camera_path.transition_duration:
        t = elapsed / camera_path.transition_duration
        t = t * t * (3.0 - 2.0 * t)  # Smoothstep

        camera_pos = current_pos * (1.0 - t) + next_pos * t
        look_target = current_target * (1.0 - t) + next_target * t
    else:
        camera_pos = next_pos
        look_target = next_target

    # Update camera transform
    for batch in view.iter_batches():
        transform = batch.column_mut(Transform)
        camera_look_at_kernel(
            transform.translation.x,
            transform.translation.y,
            transform.translation.z,
            transform.rotation.x,
            transform.rotation.y,
            transform.rotation.z,
            transform.rotation.w,
            camera_pos[0],
            camera_pos[1],
            camera_pos[2],
            look_target[0],
            look_target[1],
            look_target[2],
        )


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(CityLayout())
        .add_systems(Startup, (setup_city, spawn_people, setup_scene))
        .add_systems(
            Update, (walking_system, camera_flight_system, warning_light_pulsate_system)
        )
    )


if __name__ == "__main__":
    print(f"City simulation: {NUM_PEOPLE:,} walking people with View+Numba JIT.")
    main().run()
