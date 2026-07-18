# JAX Batch Guide

GPU-accelerated batch computation using `iter_batches()` + `@jax.jit` kernels.

For workloads where GPU parallelism outweighs copy overhead - N-body interactions,
ML inference, large matrix operations. JAX compiles Python functions to optimized
XLA code that runs on CPU or GPU.

## When to Use

See `guide://performance` for the full strategy comparison table. Use JAX when:
- You need GPU acceleration (O(n²) interactions, neural network inference)
- The computation is heavy enough that copy overhead is negligible
- You want functional/vectorized style with automatic differentiation support

**Prefer Numba over JAX when:**
- Zero-copy matters (tight per-frame budgets with simple per-entity math)
- You need imperative in-place mutation
- CPU-only is sufficient

## Setup

```bash
pip install jax jaxlib          # CPU
pip install jax[cuda12]         # GPU (NVIDIA)
```

Activate JAX support by importing the extension:
```python
import pybevy.ecs.jax_ext  # Registers pytrees + methods on ViewColumn
```

## Core Pattern

```python
import jax
import jax.numpy as jnp
import pybevy.ecs.jax_ext  # activate JAX support
from pybevy.prelude import *
from pybevy.ecs import View

# 1. Define kernel at MODULE LEVEL. XLA compiles on first call and caches.
@jax.jit
def my_kernel(pos, speed, t, dt):
    # pos is SimpleNamespace(x=, y=, z=) from Vec3ViewColumn pytree
    from types import SimpleNamespace
    new_x = pos.x + jnp.cos(speed + t) * dt
    new_y = jnp.maximum(pos.y + jnp.sin(speed + t) * dt, 0.0)
    return SimpleNamespace(x=new_x, y=new_y, z=pos.z)

# 2. System extracts ViewColumns, passes to kernel, writes back
def my_system(
    view: View[tuple[Mut[Transform], MyComponent], With[Marker]],
    time: Res[Time],
) -> None:
    t = time.elapsed_secs()
    dt = time.delta_secs()

    for batch in view.iter_batches():
        pos = batch.column_mut(Transform)
        comp = batch.column(MyComponent)

        # Read: Vec3ViewColumn auto-converts via pytree protocol
        new_pos = my_kernel(pos.translation, comp.speed, t, dt)

        # Write back: accepts SimpleNamespace with .x, .y, .z
        pos.translation.from_jax(new_pos)
```

## Data Flow

```
ECS memory → (pytree flatten: copy out) → JAX kernel → new arrays → (from_jax: copy back) → ECS memory
```

The read direction is automatic (ViewColumn registered as JAX pytree, converted on
`@jax.jit` call boundary). The write-back is always explicit via `from_jax()` because
JAX is immutable - it returns new arrays, never mutates inputs.

## API Details

### ViewColumn Methods (added by jax_ext)

```python
col.to_jax()          # → jax.Array (explicit conversion)
col.from_jax(arr)     # Write jax.Array back into ECS storage
```

### Vec3ViewColumn / QuatViewColumn Write-Back

```python
# From SimpleNamespace (matches pytree unflatten output)
pos.translation.from_jax(result)     # result has .x, .y, .z attributes

# From separate arrays
pos.translation.from_jax(new_x, new_y, new_z)

# QuatViewColumn works the same way
col.rotation.from_jax(result)        # result has .x, .y, .z, .w
col.rotation.from_jax(qx, qy, qz, qw)
```

### Transparent Pytree Conversion

ViewColumn, Vec3ViewColumn, and QuatViewColumn are registered as JAX pytrees.
When passed to `@jax.jit`, they auto-convert:

```python
@jax.jit
def kernel(x_col, y_col):     # x_col, y_col are jax.Arrays at this point
    return x_col + y_col

# ViewColumns passed directly - conversion happens automatically
result = kernel(pos.translation.x, pos.translation.y)
```

Vec3ViewColumn flattens to 3 arrays (x, y, z):
```python
@jax.jit
def kernel(translation):
    # translation is SimpleNamespace(x=array, y=array, z=array)
    return translation.x + 1.0, translation.y + 2.0, translation.z + 3.0

new_x, new_y, new_z = kernel(pos.translation)
```

### Low-Level Buffer Methods

For manual control (also useful for non-JAX frameworks):

```python
raw = col.to_contiguous_bytes()        # bytes in native dtype (f4/f8/i4/i8)
col.write_from_buffer(raw)             # write back from bytes
```

## Copy Cost

The round-trip copy cost at 60fps:

| Entities | 3 floats (position) | 7 floats (pos+vel+mass) |
|----------|--------------------|-----------------------|
| 10k | 0.005 ms | 0.01 ms |
| 100k | 0.05 ms | 0.1 ms |
| 1M | 0.5 ms | 1.1 ms |

For workloads where JAX makes sense (O(n²) interactions, ML inference), the
computation time dominates the copy cost.

## Complete Example: N-Body Gravity

All-pairs gravitational interaction - O(n²) on CPU, trivially parallel on GPU:

```python
import jax
import jax.numpy as jnp
import pybevy.ecs.jax_ext
from dataclasses import dataclass
from pybevy.prelude import *
from pybevy.ecs import View

@component
@dataclass
class Mass(Component):
    value: float = 1.0

@component
class Body(Component):
    pass

@jax.jit
def nbody_step(pos, mass, dt):
    from types import SimpleNamespace
    # pos is SimpleNamespace(x=, y=, z=) from Vec3ViewColumn pytree
    dx = pos.x[:, None] - pos.x[None, :]
    dy = pos.y[:, None] - pos.y[None, :]
    dz = pos.z[:, None] - pos.z[None, :]
    r2 = dx**2 + dy**2 + dz**2 + 1e-4  # softening
    inv_r3 = r2 ** (-1.5)

    ax = jnp.sum(dx * inv_r3 * mass[None, :], axis=1) * dt
    ay = jnp.sum(dy * inv_r3 * mass[None, :], axis=1) * dt
    az = jnp.sum(dz * inv_r3 * mass[None, :], axis=1) * dt

    return SimpleNamespace(x=pos.x + ax, y=pos.y + ay, z=pos.z + az)

def gravity_system(
    view: View[tuple[Mut[Transform], Mass], With[Body]],
    time: Res[Time],
) -> None:
    dt = time.delta_secs()
    if dt <= 0.0 or dt > 0.1:
        return

    for batch in view.iter_batches():
        pos = batch.column_mut(Transform)
        mass_col = batch.column(Mass)

        new_pos = nbody_step(pos.translation, mass_col.value, dt)
        pos.translation.from_jax(new_pos)
```

## JAX vs Numba

| | Numba | JAX |
|---|---|---|
| Copy cost | Zero (in-place mutation) | ~0.05ms/100k (round-trip) |
| GPU support | No | Yes |
| Style | Imperative loops | Functional/vectorized |
| Best for | Per-entity branching, tight budgets | O(n²) interactions, ML, GPU |
| Parallelism | `numba.prange` (CPU cores) | XLA (CPU/GPU, automatic) |

Use both in the same project - Numba for simple per-entity updates,
JAX for heavy batch computation.
