# Demoscene Tunnel Recipe

Tetris blocks cascade into a grid, morph into 3D tunnel rings, then camera flies through a twisting neon tunnel. Demonstrates: multi-phase state machine, smoothstep interpolation, entity recycling, procedural camera path.

```python
import math
from dataclasses import dataclass
from pybevy.prelude import *
from pybevy.pbr import DistanceFog, FogFalloff

# === Configuration ===
COLS, ROWS, CELL = 10, 18, 1.0
DROP_HEIGHT, DROP_SPEED, STAGGER = 25.0, 14.0, 0.06
TUNNEL_RADIUS, RING_SPACING, NUM_RINGS = 5.0, 5.0, 8
TUNNEL_LEN = NUM_RINGS * RING_SPACING
FLY_SPEED, MORPH_DUR, SETTLE_DUR = 12.0, 2.5, 1.0

COLORS = {
    "I": (0.0, 0.9, 0.9),   "O": (0.95, 0.9, 0.1),
    "T": (0.6, 0.1, 0.8),   "S": (0.1, 0.85, 0.2),
    "Z": (0.9, 0.15, 0.15), "J": (0.15, 0.2, 0.85),
    "L": (0.95, 0.55, 0.1),
}

# --- Grid layout (row 0 = bottom) ---
grid = [[None] * COLS for _ in range(ROWS)]

def place(pt, cells):
    for r, c in cells:
        grid[r][c] = pt

place("I", [(0,0),(0,1),(0,2),(0,3)])
place("O", [(0,4),(0,5),(1,4),(1,5)])
place("L", [(0,6),(0,7),(0,8),(1,8)])
place("J", [(1,0),(1,1),(1,2),(1,6)])
place("T", [(1,7),(2,6),(2,7),(2,8)])
place("Z", [(1,3),(2,2),(2,3)])
place("S", [(2,0),(2,1),(3,1),(3,2)])
place("I", [(2,4),(2,5),(2,9),(3,9)])
place("O", [(3,3),(3,4),(4,3),(4,4)])
place("L", [(3,5),(3,6),(3,7),(3,8)])
place("Z", [(4,0),(4,1),(5,1),(5,2)])
place("J", [(4,7),(4,8),(4,9),(5,7)])
place("T", [(4,5),(4,6),(5,5),(5,6)])
place("I", [(5,3),(5,4),(4,2)])
place("S", [(6,1),(6,2),(7,0),(7,1)])
place("L", [(6,5),(6,6),(6,7),(7,7)])
place("O", [(6,8),(6,9),(7,8),(7,9)])
place("Z", [(7,3),(7,4),(8,4),(8,5)])
place("T", [(8,0),(8,1),(8,2),(9,1)])
place("J", [(8,7),(8,8),(8,9),(9,9)])
place("T", [(10,4),(10,5),(10,6),(11,5)])

# --- Pre-compute tunnel ring assignments ---
# Each block is assigned a ring index and angle within that ring.
# Round-robin assignment distributes blocks evenly across rings.
block_list = [(r, c) for r in range(ROWS) for c in range(COLS) if grid[r][c] is not None]

ring_buckets: dict[int, list] = {i: [] for i in range(NUM_RINGS)}
for i, (row, col) in enumerate(block_list):
    ring_buckets[i % NUM_RINGS].append((row, col))

tunnel_map: dict[tuple[int, int], tuple[int, float]] = {}
for ring_idx, cells in ring_buckets.items():
    n = len(cells)
    for j, (row, col) in enumerate(cells):
        tunnel_map[(row, col)] = (ring_idx, (j / n) * math.tau)


# === Helpers ===

def smoothstep(t: float) -> float:
    """Hermite interpolation — ease-in-out between 0 and 1."""
    t = max(0.0, min(1.0, t))
    return t * t * (3.0 - 2.0 * t)


def tunnel_center(z: float, t: float) -> tuple[float, float]:
    """Wobbling tunnel centerline. Returns (x, y) at depth z and time t."""
    cx = math.sin(z * 0.12 + t * 0.7) * 2.5
    cy = math.cos(z * 0.09 + t * 0.4) * 1.8 + 5.0
    return cx, cy


def ring_z_pos(ring_idx: int, cam_z: float, wrap: bool) -> float:
    """Z position of a ring. With wrap=True, recycles rings ahead of camera."""
    base_z = -ring_idx * RING_SPACING
    if not wrap:
        return base_z
    # Modular arithmetic keeps rings cycling ahead of the camera
    ahead = (cam_z - base_z) % TUNNEL_LEN
    if ahead < 1.0:
        ahead += TUNNEL_LEN
    return cam_z - ahead


def block_pos(ring_idx: int, angle: float, cam_z: float, t: float,
              wrap: bool, rot_scale: float) -> tuple[float, float, float]:
    """World position for a tunnel block."""
    rz = ring_z_pos(ring_idx, cam_z, wrap)
    a = angle + t * rot_scale * (0.5 + ring_idx * 0.15)  # per-ring rotation speed
    cx, cy = tunnel_center(rz, t)
    r = TUNNEL_RADIUS + math.sin(rz * 0.2 + t * 0.8) * 0.6  # radius pulsing
    return cx + r * math.cos(a), cy + r * math.sin(a), rz


# === ECS ===

@component
@dataclass
class TetrisBlock(Component):
    grid_x: float = 0.0       # X position in the tetris grid
    target_y: float = 0.0     # Y position when landed
    delay: float = 0.0        # stagger delay before drop starts
    landed: bool = False
    col: int = 0
    row: int = 0
    ring_idx: int = 0         # tunnel ring assignment
    ring_angle: float = 0.0   # angle within ring


@resource
@dataclass
class ScenePhase(Resource):
    phase: str = "drop"       # drop -> settle -> morph -> fly
    timer: float = 0.0
    cam_z: float = 5.0


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, animate_blocks)
        .add_systems(Update, animate_camera)
    )


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    ox = -COLS * CELL / 2.0 + CELL / 2.0

    # Camera — high bloom for neon glow, tight dark fog
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, ROWS * 0.45, COLS * 1.4)
            .looking_at(Vec3(0, ROWS * 0.3, 0), Vec3.Y),
        Bloom(intensity=0.35, low_frequency_boost=0.7),
        DistanceFog(
            color=Color.srgb(0.01, 0.01, 0.04),
            falloff=FogFalloff.Exponential(0.02),
            directional_light_color=Color.srgb(0.5, 0.4, 0.7),
            directional_light_exponent=15.0,
        ),
        Name("camera"),
    )

    commands.spawn(
        DirectionalLight(illuminance=5000.0, color=Color.srgb(1.0, 0.95, 0.9),
                         shadow_maps_enabled=True),
        Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.9, 0.5, 0.0)),
    )
    commands.insert_resource(GlobalAmbientLight(brightness=200.0, color=Color.srgb(0.4, 0.4, 0.7)))
    commands.insert_resource(ClearColor(Color.srgb(0.005, 0.005, 0.02)))
    commands.insert_resource(ScenePhase())

    cube_mesh = meshes.add(Cuboid(CELL * 0.88, CELL * 0.88, CELL * 0.88))

    # Emissive materials for neon glow through bloom
    mat_cache: dict[str, object] = {}
    for name, (r, g, b) in COLORS.items():
        mat_cache[name] = materials.add(StandardMaterial(
            base_color=Color.srgb(r, g, b),
            emissive=Color.linear_rgb(r * 1.5, g * 1.5, b * 1.5),
            metallic=0.05, perceptual_roughness=0.25,
        ))

    # Spawn blocks above their targets — they'll drop in
    idx = 0
    for row in range(ROWS):
        for col in range(COLS):
            piece = grid[row][col]
            if piece is None:
                continue
            x = ox + col * CELL
            target_y = row * CELL + CELL / 2.0
            ring_idx, ring_angle = tunnel_map[(row, col)]
            commands.spawn(
                Mesh3d(cube_mesh), MeshMaterial3d(mat_cache[piece]),
                Transform.from_xyz(x, target_y + DROP_HEIGHT, 0.0),
                TetrisBlock(grid_x=x, target_y=target_y, delay=idx * STAGGER,
                            col=col, row=row, ring_idx=ring_idx, ring_angle=ring_angle),
            )
            idx += 1

    # Border walls (left, right, bottom)
    wall_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.1, 0.1, 0.15), metallic=0.5, perceptual_roughness=0.6))
    wt = 0.15
    wall_mesh = meshes.add(Cuboid(wt, ROWS * CELL, CELL))
    commands.spawn(Mesh3d(wall_mesh), MeshMaterial3d(wall_mat),
                   Transform.from_xyz(ox - CELL/2.0 - wt/2.0, ROWS*CELL/2.0, 0.0))
    commands.spawn(Mesh3d(wall_mesh), MeshMaterial3d(wall_mat),
                   Transform.from_xyz(-ox + CELL/2.0 + wt/2.0, ROWS*CELL/2.0, 0.0))
    bot_mesh = meshes.add(Cuboid(COLS * CELL + wt * 2, wt, CELL))
    commands.spawn(Mesh3d(bot_mesh), MeshMaterial3d(wall_mat),
                   Transform.from_xyz(0.0, -wt/2.0, 0.0))


def animate_blocks(
    query: Query[tuple[Mut[Transform], Mut[TetrisBlock]]],
    time: Res[Time],
    state: ResMut[ScenePhase],
) -> None:
    t = time.elapsed_secs()
    dt = time.delta_secs()

    if state.phase == "drop":
        # Blocks fall with staggered delays, wave-bob after landing
        all_landed = True
        for transform, block in query:
            if not block.landed:
                if t < block.delay:
                    all_landed = False
                    continue
                new_y = transform.translation.y - DROP_SPEED * dt
                if new_y <= block.target_y:
                    transform.translation.y = block.target_y
                    block.landed = True
                else:
                    transform.translation.y = new_y
                    all_landed = False
            else:
                p = block.col * 0.7 + block.row * 0.4
                transform.translation.y = block.target_y + math.sin(t * 2.5 + p) * 0.06
        if all_landed:
            state.phase = "settle"
            state.timer = t

    elif state.phase == "settle":
        # Brief pause with gentle wave animation
        for transform, block in query:
            p = block.col * 0.7 + block.row * 0.4
            transform.translation.y = block.target_y + math.sin(t * 2.5 + p) * 0.06
        if t - state.timer >= SETTLE_DUR:
            state.phase = "morph"
            state.timer = t

    elif state.phase == "morph":
        # Smoothstep lerp from grid positions to tunnel ring positions
        sm = smoothstep((t - state.timer) / MORPH_DUR)
        for transform, block in query:
            sx, sy, sz = block.grid_x, block.target_y, 0.0
            tx, ty, tz = block_pos(block.ring_idx, block.ring_angle,
                                   state.cam_z, t, wrap=False, rot_scale=sm * 0.3)
            transform.translation.x = sx + (tx - sx) * sm
            transform.translation.y = sy + (ty - sy) * sm
            transform.translation.z = sz + (tz - sz) * sm
        if sm >= 1.0:
            state.phase = "fly"
            state.timer = t

    elif state.phase == "fly":
        # Camera advances, blocks positioned in recycling tunnel rings
        fly_t = t - state.timer
        state.cam_z -= (FLY_SPEED + fly_t * 0.8) * dt  # gradual acceleration
        for transform, block in query:
            x, y, z = block_pos(block.ring_idx, block.ring_angle,
                                state.cam_z, t, wrap=True, rot_scale=1.0)
            transform.translation.x = x
            transform.translation.y = y
            transform.translation.z = z


def animate_camera(
    query: Query[Mut[Transform], With[Camera3d]],
    time: Res[Time],
    state: Res[ScenePhase],
) -> None:
    t = time.elapsed_secs()

    if state.phase in ("drop", "settle"):
        return  # camera stays at initial position

    if state.phase == "morph":
        # Smoothstep from tetris view to tunnel entry
        sm = smoothstep((t - state.timer) / MORPH_DUR)
        sx, sy, sz = 0.0, ROWS * 0.45, COLS * 1.4
        slx, sly, slz = 0.0, ROWS * 0.3, 0.0
        cz = state.cam_z
        cx, cy = tunnel_center(cz, t)
        lz = cz - 20.0
        lcx, lcy = tunnel_center(lz, t)
        for transform in query:
            transform.translation = Vec3(
                sx + (cx - sx) * sm, sy + (cy - sy) * sm, sz + (cz - sz) * sm,
            )
            transform.look_at(Vec3(
                slx + (lcx - slx) * sm, sly + (lcy - sly) * sm, slz + (lz - slz) * sm,
            ), Vec3.Y)

    elif state.phase == "fly":
        # Follow the tunnel centerline, look ahead
        cz = state.cam_z
        cx, cy = tunnel_center(cz, t)
        lz = cz - 25.0
        lcx, lcy = tunnel_center(lz, t)
        for transform in query:
            transform.translation = Vec3(cx, cy, cz)
            transform.look_at(Vec3(lcx, lcy, lz), Vec3.Y)


if __name__ == "__main__":
    main().run()
```

## Key Techniques

### Multi-Phase State Machine
A `@resource` tracks the current phase (`drop` / `settle` / `morph` / `fly`) and a timer. Each phase checks its exit condition and transitions to the next. This is cleaner than a single monolithic system with many time checks.

### Smoothstep & Easing

`smoothstep(t) = t*t*(3 - 2*t)` gives ease-in-out motion. Use it anywhere you lerp between two states:

```python
def smoothstep(t: float) -> float:
    t = max(0.0, min(1.0, t))
    return t * t * (3.0 - 2.0 * t)

sm = smoothstep(elapsed / duration)
value = start + (end - start) * sm
```

Other common easing curves:
```python
t * t                            # ease-in (slow start)
1.0 - (1.0 - t) * (1.0 - t)    # ease-out (slow end)
1.0 - math.exp(-5.0 * t)        # exponential ease-out (snappy)
```

`Vec3.lerp()` is built-in: `pos = start_pos.lerp(end_pos, t)`

### Entity Recycling (Infinite Tunnel)
Only ~75 blocks exist, but the tunnel feels infinite. Modular arithmetic wraps ring Z positions ahead of the camera as it advances:
```python
ahead = (cam_z - base_z) % TUNNEL_LEN
if ahead < 1.0:       # too close — push to next cycle
    ahead += TUNNEL_LEN
ring_z = cam_z - ahead
```
When a ring falls behind the camera, it silently jumps ahead. Works for any infinite scroller, tunnel, or looping background.

### Camera Path-Following
The camera follows a procedural curve (`tunnel_center`) rather than predefined keyframes. Set the position, then use `look_at()` in-place:
```python
transform.translation = Vec3(x, y, z)
transform.look_at(target, Vec3.Y)
```
`look_at()` modifies the transform's rotation in-place. `looking_at()` is the builder variant for owned transforms (e.g., in Startup systems).

### Staggered Drop Animation
Each block has a `delay` field = `index * STAGGER`. In the Update system, blocks wait until `elapsed > delay` before starting to fall. This creates the satisfying cascade effect with zero complexity.
