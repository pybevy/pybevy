# Numba Batch Guide

High-performance per-entity loops using `iter_batches()` + `@numba.jit` kernels.

For 100k+ entities where the View expression API isn't fast enough, Numba compiles
Python loops to native LLVM code with zero-copy access to Bevy's archetype storage.

## When to Use

See `guide://performance` for the full strategy comparison table. Use Numba when:
- View expressions aren't fast enough (profiling shows > 5ms)
- You need complex per-entity branching that `.where()` can't express
- You want `parallel=True` to spread work across CPU cores

## Core Pattern

```python
import math
import numba
from pybevy.prelude import *
from pybevy.ecs import View

# 1. Define kernel at MODULE LEVEL (not inside the system function).
#    Numba compiles on first call and caches to disk.
@numba.jit(nopython=True, parallel=True, fastmath=True, cache=True)
def my_kernel(x, y, z, speed, t, dt):
    for i in numba.prange(len(x)):      # prange for parallel threads
        x[i] += math.cos(speed[i] + t) * dt
        y[i] += math.sin(speed[i] + t) * dt
        if y[i] < 0.0:                  # per-entity branching OK
            y[i] = 0.0

# 2. System extracts ViewColumns and passes them to the kernel
def my_system(
    view: View[tuple[Mut[Transform], MyComponent], With[Marker]],
    time: Res[Time],
) -> None:
    t = time.elapsed_secs()
    dt = time.delta_secs()

    for batch in view.iter_batches():
        pos = batch.column_mut(Transform)
        comp = batch.column(MyComponent)

        # Pass individual scalar ViewColumns to the kernel
        my_kernel(
            pos.translation.x, pos.translation.y, pos.translation.z,
            comp.speed,
            t, dt,
        )
```

## API Details

### iter_batches()

`view.iter_batches()` yields one `Batch` per archetype. Each batch contains
entities with identical component sets, stored contiguously in memory.

Typically there is **one batch** for all entities of the same type (same
components). Multiple batches occur when entities have different component
combinations.

### Batch.column() / Batch.column_mut()

```python
batch.column(Transform)      # read-only TransformViewColumn
batch.column_mut(Transform)  # mutable TransformViewColumn
batch.column(MyComponent)    # read-only ViewColumn with dynamic field access
batch.column_mut(MyComponent)  # mutable ViewColumn
```

### Accessing Fields

**Transform** (returns Vec3ViewColumn / QuatViewColumn):
```python
pos = batch.column_mut(Transform)
pos.translation.x   # ViewColumn for x coordinates
pos.translation.y   # ViewColumn for y coordinates
pos.translation.z   # ViewColumn for z coordinates
pos.scale.x         # ViewColumn for scale x
pos.rotation.x      # ViewColumn for quaternion x
```

**Custom components** (returns ViewColumn with dynamic field access):
```python
bird = batch.column(BirdParam)
bird.phase    # ViewColumn for the 'phase' field
bird.layer    # ViewColumn for the 'layer' field
bird.speed    # ViewColumn for the 'speed' field
```

### ViewColumn in Numba

ViewColumn is an **opaque handle**. You CANNOT convert it to numpy.
The only way to access data is through `[]` indexing inside `@numba.jit`:

```python
@numba.jit(nopython=True)
def kernel(col):
    for i in range(len(col)):
        col[i] = col[i] + 1.0    # read and write via []
```

**Do NOT:**
- `np.asarray(col)` — raises RuntimeError
- `col.to_numpy()` — does not exist on ViewColumn
- `col[0]` in Python — only works inside Numba JIT
- Cache ViewColumns in globals — they become stale after the system returns

**Debugging (Python-side):**
- `col.peek(0)` — read single value (safe, with validity check)
- `col.to_list()` — convert to Python list (copies data, for debugging only)
- `col.is_valid` — check if handle is still alive

## Numba JIT Options

```python
@numba.jit(
    nopython=True,    # Required: no Python fallback
    parallel=True,    # Split loop across CPU cores (use numba.prange)
    fastmath=True,    # Faster trig/sqrt (slight precision trade-off)
    cache=True,       # Cache compiled code to disk (avoids recompile on restart)
)
def kernel(x, y, z, t, dt):
    for i in numba.prange(len(x)):  # parallel iteration
        ...
```

- `parallel=True` + `numba.prange`: biggest win for 100k+ entities
- `fastmath=True`: ~10-20% faster trig, invisible precision loss for games
- `cache=True`: first call compiles (~1-2s), subsequent app starts use cache
- Always use `range()` or `numba.prange()` — never iterate ViewColumn directly

## Complete Example: Murmuration

100k birds with cohesion, 3-layer flow field, and floor clamping at 1.2ms/frame:

```python
import math
import numba
from pybevy.prelude import *
from pybevy.ecs import View

@component
@dataclass
class BirdParam(Component):
    phase: float = 0.0
    layer: float = 0.0
    speed: float = 1.0

@numba.jit(nopython=True, parallel=True, fastmath=True, cache=True)
def murmuration_kernel(tx, ty, tz, phase, layer, speed, t, dt, cx, cy, cz, max_r):
    for i in numba.prange(len(tx)):
        # Cohesion: pull toward drifting center
        rx = tx[i] - cx
        ry = ty[i] - cy
        rz = tz[i] - cz
        dist = math.sqrt(rx * rx + ry * ry + rz * rz + 0.1)
        excess = max(dist - max_r, 0.0)
        pull = (0.3 + excess * 0.2) * dt
        inv_dist = 1.0 / dist
        tx[i] -= rx * inv_dist * pull
        ty[i] -= ry * inv_dist * pull
        tz[i] -= rz * inv_dist * pull

        # Flow field layers (sin/cos compile to native LLVM intrinsics)
        s1 = t * 0.4
        vx = math.cos(ty[i] * 0.1 + s1) * 4.0
        vy = math.sin(tz[i] * 0.1 + s1 * 0.7) * 2.0
        vz = -math.sin(tx[i] * 0.1 + s1 * 0.5) * 4.0

        spd = dt * speed[i]
        tx[i] += vx * spd
        ty[i] += vy * spd
        tz[i] += vz * spd

        # Floor clamp
        if ty[i] < 4.0:
            ty[i] = 4.0

def murmuration(
    view: View[tuple[Mut[Transform], BirdParam], With[Starling]],
    time: Res[Time],
) -> None:
    t = time.elapsed_secs()
    dt = time.delta_secs()
    if dt <= 0.0 or dt > 0.1:
        return

    cx = math.sin(t * 0.07) * 12.0
    cy = 30.0 + math.sin(t * 0.11) * 4.0
    cz = math.cos(t * 0.05) * 8.0
    max_r = 12.0 + 4.0 * math.sin(t * 0.18)

    for batch in view.iter_batches():
        pos = batch.column_mut(Transform)
        bird = batch.column(BirdParam)
        murmuration_kernel(
            pos.translation.x, pos.translation.y, pos.translation.z,
            bird.phase, bird.layer, bird.speed,
            t, dt, cx, cy, cz, max_r,
        )
```

## Stub Caveats

The `.pyi` stubs have some inaccuracies for the batch path:

- `FieldExpr.to_numpy()` and `Vec3Expr.to_numpy()` are documented
  in the stubs but **do not work** in batch context. `batch.column()` returns
  `ViewColumn` types at runtime, not `FieldExpr`/`Vec3Expr`.
- The `iter_batches()` docstring references `batch.column_numpy()` which
  **does not exist** on the `Batch` class.
- The `ViewColumn` docstring references `batch.col()` — the correct method
  name is `batch.column()` / `batch.column_mut()`.

The working pattern is always: extract scalar ViewColumns -> pass to Numba kernel -> use `[]` indexing.

## Performance Comparison

Benchmark: 100k entities, sunset murmuration scene

| Method | Time | Notes |
|--------|------|-------|
| Query loop | ~650ms | Python iteration overhead |
| View expressions | ~13ms | Vectorized Rust, no per-entity branching |
| Numba (serial) | ~4ms | `nopython=True, cache=True` |
| Numba (parallel+fastmath) | ~1.2ms | `parallel=True, fastmath=True` |
