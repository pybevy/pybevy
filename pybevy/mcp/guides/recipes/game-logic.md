# Game Logic Recipe — Grid Movement + Simple AI

Grid-based movement with smooth interpolation, BFS pathfinding, and flee/chase AI. Good for maze games, puzzle games, tactics, or any tile-based scene.

## Core Pattern

Entities move on a grid. A `GridMover` component tracks current cell, target cell, and interpolation progress. An Update system smoothly moves the Transform between cells.

```python
import math
import random
from collections import deque
from dataclasses import dataclass
from pybevy.prelude import *

CELL = 2.0  # world units per grid cell

# Grid helpers

MAZE = [
    [1,1,1,1,1],
    [1,0,0,0,1],
    [1,0,1,0,1],
    [1,0,0,0,1],
    [1,1,1,1,1],
]
ROWS = len(MAZE)
COLS = len(MAZE[0])


def is_open(gx: int, gz: int) -> bool:
    return 0 <= gz < ROWS and 0 <= gx < COLS and MAZE[gz][gx] == 0


def get_neighbors(gx: int, gz: int) -> list:
    result = []
    for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
        nx, nz = gx + dx, gz + dz
        if is_open(nx, nz):
            result.append((nx, nz))
    return result


# Components

@component
@dataclass
class GridMover(Component):
    gx: int = 1           # current grid x
    gz: int = 1           # current grid z
    target_gx: int = 1    # destination grid x
    target_gz: int = 1    # destination grid z
    progress: float = 1.0  # 0→1 interpolation (1.0 = arrived, ready for new target)
    speed: float = 2.5     # cells per second

@component
class Chaser(Component):
    pass

@component
class Prey(Component):
    pass
```

## Movement System

Smoothly interpolates Transform between grid cells with a hop bounce:

```python
def move_entities(
    query: Query[tuple[Mut[Transform], Mut[GridMover]]],
    time: Res[Time],
) -> None:
    dt = time.delta_secs()
    for transform, mover in query:
        if mover.progress < 1.0:
            mover.progress = min(1.0, mover.progress + dt * mover.speed)

            sx = float(mover.gx) * CELL
            sz = float(mover.gz) * CELL
            tx = float(mover.target_gx) * CELL
            tz = float(mover.target_gz) * CELL

            # Smoothstep interpolation
            t = mover.progress
            t_smooth = t * t * (3.0 - 2.0 * t)
            transform.translation.x = sx + (tx - sx) * t_smooth
            transform.translation.z = sz + (tz - sz) * t_smooth

            # Hop bounce
            transform.translation.y = math.sin(t * math.pi) * 0.15

            # Face movement direction
            dx = tx - sx
            dz = tz - sz
            if abs(dx) > 0.01 or abs(dz) > 0.01:
                angle = math.atan2(dx, dz)
                transform.rotation = Quat.from_euler(EulerRot.XYZ, 0.0, angle, 0.0)

        if mover.progress >= 1.0:
            mover.gx = mover.target_gx
            mover.gz = mover.target_gz
            transform.translation.y = 0.0
```

**Key points:**
- AI systems only set new targets when `progress >= 1.0` (entity has arrived)
- Movement system only advances when `progress < 1.0`
- This naturally prevents AI and movement from conflicting

## BFS Pathfinding

Returns the first step on the shortest path. Run once per cell arrival, not every frame:

```python
def bfs_next_step(fx: int, fz: int, tx: int, tz: int) -> tuple:
    """Return the first grid step on the shortest path from (fx,fz) to (tx,tz)."""
    if fx == tx and fz == tz:
        return fx, fz
    visited = set()
    visited.add((fx, fz))
    queue = deque()
    for nx, nz in get_neighbors(fx, fz):
        queue.append((nx, nz, nx, nz))  # (current_x, current_z, first_step_x, first_step_z)
        visited.add((nx, nz))
    while queue:
        cx, cz, first_x, first_z = queue.popleft()
        if cx == tx and cz == tz:
            return first_x, first_z
        for nx, nz in get_neighbors(cx, cz):
            if (nx, nz) not in visited:
                visited.add((nx, nz))
                queue.append((nx, nz, first_x, first_z))
    nbrs = get_neighbors(fx, fz)
    return nbrs[0] if nbrs else (fx, fz)
```

## Cross-Entity AI (Avoiding Query Conflicts)

A chaser needs to read prey positions, but both share `GridMover`. You **cannot** mix `Mut[GridMover]` and `GridMover` in one system without `Without` filters.

**Simple case (2 entity types):** Add `Without` filters to prove disjointness — keeps everything in one system:

```python
# ✅ Without filters → zero-cost, no extra systems
def chaser_ai(
    chasers: Query[Mut[GridMover], tuple[With[Chaser], Without[Prey]]],
    prey: Query[GridMover, tuple[With[Prey], Without[Chaser]]],
) -> None:
    prey_positions = [(m.gx, m.gz) for m in prey]
    for mover in chasers:
        # BFS toward nearest prey...
```

**Complex case (3+ systems need the same data):** Route through a resource to avoid duplicating reads:

```python
@resource
@dataclass
class GameState(Resource):
    # Chaser position (written by sync, read by prey AI)
    chaser_gx: int = 1
    chaser_gz: int = 1
    # Prey positions — flat fields (convenient for small fixed counts)
    p0x: int = 0
    p0z: int = 0
    p1x: int = 0
    p1z: int = 0

# System 1: Sync all positions into resource (two IMMUTABLE queries — OK)
def sync_positions(
    chaser_q: Query[GridMover, With[Chaser]],
    prey_q: Query[GridMover, With[Prey]],
    state: ResMut[GameState],
) -> None:
    for m in chaser_q:
        state.chaser_gx = m.gx
        state.chaser_gz = m.gz
    positions = []
    for m in prey_q:
        positions.append((m.gx, m.gz))
    if len(positions) > 0:
        state.p0x = positions[0][0]
        state.p0z = positions[0][1]
    if len(positions) > 1:
        state.p1x = positions[1][0]
        state.p1z = positions[1][1]

# System 2: Chaser AI — BFS toward nearest prey
def chaser_ai(
    query: Query[Mut[GridMover], With[Chaser]],
    state: Res[GameState],
) -> None:
    prey_positions = [(state.p0x, state.p0z), (state.p1x, state.p1z)]
    for mover in query:
        if mover.progress < 1.0:
            continue
        # Find nearest prey
        best_px, best_pz = mover.gx, mover.gz
        best_d = 999999.0
        for px, pz in prey_positions:
            dx = float(px - mover.gx)
            dz = float(pz - mover.gz)
            d = dx * dx + dz * dz
            if d < best_d:
                best_d = d
                best_px = px
                best_pz = pz
        next_gx, next_gz = bfs_next_step(mover.gx, mover.gz, best_px, best_pz)
        mover.target_gx = next_gx
        mover.target_gz = next_gz
        mover.progress = 0.0

# System 3: Prey AI — flee when close, wander otherwise
def prey_ai(
    query: Query[Mut[GridMover], With[Prey]],
    state: Res[GameState],
) -> None:
    for mover in query:
        if mover.progress < 1.0:
            continue
        nbrs = get_neighbors(mover.gx, mover.gz)
        if not nbrs:
            continue
        sdx = float(state.chaser_gx - mover.gx)
        sdz = float(state.chaser_gz - mover.gz)
        sq_dist = sdx * sdx + sdz * sdz
        if sq_dist < 25.0:
            # Flee: pick neighbor furthest from chaser
            best = max(nbrs, key=lambda n: (n[0] - state.chaser_gx)**2 + (n[1] - state.chaser_gz)**2)
            mover.target_gx = best[0]
            mover.target_gz = best[1]
        else:
            # Wander randomly
            choice = random.choice(nbrs)
            mover.target_gx = choice[0]
            mover.target_gz = choice[1]
        mover.progress = 0.0
```

## Catch / Collision Detection

Check if two entities occupy the same cell. Teleport the caught entity to a random distant cell:

```python
def check_catch(
    query: Query[tuple[Mut[Transform], Mut[GridMover]], With[Prey]],
    state: ResMut[GameState],
) -> None:
    for tf, mv in query:
        if mv.gx == state.chaser_gx and mv.gz == state.chaser_gz:
            # Respawn at random distant open cell
            for _ in range(200):
                new_gx = random.randint(0, COLS - 1)
                new_gz = random.randint(0, ROWS - 1)
                if is_open(new_gx, new_gz):
                    dist = abs(new_gx - state.chaser_gx) + abs(new_gz - state.chaser_gz)
                    if dist > 6:
                        mv.gx = new_gx
                        mv.gz = new_gz
                        mv.target_gx = new_gx
                        mv.target_gz = new_gz
                        mv.progress = 1.0
                        tf.translation.x = float(new_gx) * CELL
                        tf.translation.z = float(new_gz) * CELL
                        tf.translation.y = 0.0
                        break
```

## System Registration Order

```python
.add_systems(Update, (
    sync_positions,   # 1. Read all positions into resource
    move_entities,    # 2. Interpolate transforms
    chaser_ai,        # 3. Pick chaser targets (reads resource)
    prey_ai,          # 4. Pick prey targets (reads resource)
    check_catch,      # 5. Detect catches (reads resource)
))
```

## Speed Tuning

| Chaser speed | Prey speed | Feel |
|--------------|------------|------|
| 2.5 | 2.5 | Fair — chaser wins via BFS but prey can escape |
| 3.0 | 2.2 | Chaser has clear advantage, catches frequently |
| 3.5 | 2.0 | Quick catches, more arcade feel |
| 2.2 | 2.5 | Prey faster — chaser must corner them |

**Tip:** BFS gives the chaser a huge advantage even at equal speeds since it always takes the optimal path. Make the prey slightly faster or add flee distance to compensate.

## Maze Design Tips

- Always leave column 1 and column N-2 open as side corridors for circulation
- Bottom and top rows (inside walls) open as cross-corridors
- Avoid dead-end pockets longer than 2 cells — prey gets trapped easily
- Verify connectivity: every open cell should be reachable from every other
