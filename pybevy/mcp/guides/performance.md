# Performance Guide

Batch operations, View API, material caching, and strategy comparison for large entity counts.

## Asset Handle Caching

Reuse mesh/material handles to avoid rebuilding and uploading duplicates.

```python
# ❌ BAD - rebuilds and re-uploads the same asset every frame
def spawn_effect(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    mesh = meshes.add(Sphere(0.5))          # Re-tessellated and re-uploaded
    mat = materials.add(StandardMaterial(    # Same again
        emissive=LinearRgba.rgb(10.0, 5.0, 20.0),
    ))
    commands.spawn(Mesh3d(mesh), MeshMaterial3d(mat), ...)
```

Pre-create handles in Startup and store them in a resource:

```python
from dataclasses import dataclass

@resource
@dataclass
class VfxAssets(Resource):
    bolt_mesh: Handle[Mesh]
    bolt_mat: Handle[StandardMaterial]
    burst_mesh: Handle[Mesh]
    burst_mat: Handle[StandardMaterial]

def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    bolt_mesh = meshes.add(Cuboid(0.18, 35.0, 0.18))
    bolt_mat = materials.add(StandardMaterial(
        base_color=Color.linear_rgb(60.0, 30.0, 120.0),
        unlit=True, alpha_mode=AlphaMode.Add(),
    ))
    burst_mesh = meshes.add(Sphere(0.8))
    burst_mat = materials.add(StandardMaterial(
        base_color=Color.linear_rgb(30.0, 15.0, 60.0),
        unlit=True, alpha_mode=AlphaMode.Add(),
    ))
    commands.insert_resource(VfxAssets(bolt_mesh, bolt_mat, burst_mesh, burst_mat))

# ✅ GOOD - reuses cached handles, zero asset growth
def spawn_effect(commands: Commands, assets: Res[VfxAssets]) -> None:
    commands.spawn(Mesh3d(assets.bolt_mesh), MeshMaterial3d(assets.bolt_mat), ...)

@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, spawn_effect)
    )
```

**Key points:**
- `Assets.add()` returns a typed `Handle[T]`, not an integer ID.
- Startup inserts `VfxAssets` after constructing valid handles; it is then available to Update systems.
- `handle.id()` returns a non-owning `AssetId[T]`; use it for lookup or comparison, not to keep the asset alive.

**Applies to:** projectiles, VFX, particles, pooled enemies - anything spawned more than once.

**Verify:** `get_performance` Assets counts distinct stored assets; inspect
retained strong handles if counts keep rising.

## Material Mutation Caching

`materials.get_mut(handle)` is lazy: inspection alone does not mark the asset
changed. Assigning a field every frame does, however, re-prepare shared
material state even when the assigned value is unchanged. Check first and only
request mutable access when a write is needed.

```python
from dataclasses import dataclass

@resource
@dataclass
class MatHandle(Resource):
    value: Handle[StandardMaterial]

# ❌ BAD - unconditionally marks the material changed every frame
def animate(materials: ResMut[Assets[StandardMaterial]], handle: Res[MatHandle]):
    mat = materials.get_mut(handle.value)
    mat.base_color = Color.linear_rgb(...)  # Every frame!

# ✅ GOOD - only update on change
def animate(materials: ResMut[Assets[StandardMaterial]], handle: Res[MatHandle]):
    new_color = compute_color(time)
    current = materials.get(handle.value)
    if current is not None and current.base_color != new_color:
        mat = materials.get_mut(handle.value)
        mat.base_color = new_color
```

## Entity Count Guidelines

Typical frame rates (RTX 5090, 4K):
- Up to 10k entities: No special considerations needed
- 10k–80k entities: ~80 FPS, use View API for bulk operations
- 80k–150k entities: ~43 FPS, CPU render extraction becomes bottleneck

## Occlusion Culling

GPU occlusion culling can help scenes with substantial opaque depth complexity.
Measure before enabling it because the culling pass has its own overhead.

```python
from pybevy.camera import DepthPrepass
from pybevy.render import OcclusionCulling

commands.spawn(
    Camera3d(),
    DepthPrepass(),
    OcclusionCulling(),
    Transform.from_xyz(8.0, 6.0, 8.0).looking_at(Vec3.ZERO, Vec3.Y),
)
```

`DepthPrepass()` is required on the same camera. Do not add
`OcclusionCulling()` by itself; the incomplete configuration can prevent the
camera from rendering.

## Batch Spawning

For spawning many entities at once (1k+), use `spawn_batch` with NumPy arrays instead of looping `commands.spawn()`:

```python
import numpy as np

def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    n = 10_000

    positions = np.random.uniform(-50, 50, (n, 3)).astype(np.float32)
    mesh = meshes.add(Sphere(0.5))
    mat = materials.add(StandardMaterial(base_color=Color.srgb(0.9, 0.3, 0.15)))

    commands.spawn_batch(
        Transform.batch(translation=positions),
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        PointLight(intensity=500.0),  # Uniform: cloned to all entities
    )
```

**Key points:**
- `batch()` returns a `Batchable` - an opaque batch object consumed by `spawn_batch`
- Uniform components (plain instances like `PointLight(intensity=500.0)`) are cloned to every entity
- Works with regular system `Commands` (deferred) or `World.commands()` (immediate)
- The NumPy/component form returns `list[Entity]` via `World.commands()` and `None` via system `Commands`
- Immediate `spawn_batch` performs one asset-safety check for the complete
  prepared batch. Release wrappers returned by `Assets.get()` or
  `Assets.get_mut()`, and close zero-copy views, before immediate world
  structural operations.
- Arrays are auto-cast to float32 and validated for shape at `batch()` time.
- Arrays are auto-cast to float32 and validated for shape and declared field constraints at `from_numpy()` time; CascadeShadowConfig checks overlap in [0, 1) and non-negative minimum distance.
  Transform translation, rotation, and scale arrays also reject NaN and infinity.
- `Transform.batch()` accepts `translation` (Nx3), `rotation` (Nx4), `scale` (Nx3) - all optional
- Any Rust component with `view_fields` supports `batch()` (e.g., `PointLight.batch(intensity=arr)`)
- Custom `@component` classes with wrapper storage also support `batch()`
- Use View API afterwards for bulk per-entity updates (see below)
- To keep one shared material while varying a small shader-side integer, batch
  `MeshTag.batch(value=...)`; see `guide://shaders`.

**Limitations:**
- `batch()` requires wrapper storage - custom `@component` classes with `storage="python"` do not support it
- Python-storage query rows are O(1) shallow proxies rather than copies of the
  nested object graph. They declare exclusive scheduler access, so prefer
  wrapper storage when read parallelism or View execution matters.
- All non-Batchable components are cloned uniformly to every entity. To vary materials across batched entities, call `spawn_batch` once per material with the appropriate subset of positions.

**Legacy iterable path** still works for small batches:
```python
commands.spawn_batch([(Transform.from_xyz(i, 0, 0), Name(f"e{i}")) for i in range(100)])
```

The iterable is fully consumed and its component values are prepared before
any entity is created. An iteration, conversion, or validation error therefore
creates no partial prefix of the batch.

## Choosing a Batch Strategy

Query iteration is slowest, View expressions far faster, a compiled Numba kernel
faster still; the multiplier depends on workload and hardware, so measure.

| Approach | Entity Count | Use Case |
|----------|-------------|----------|
| Query iteration | < 1k | Method calls, simple logic |
| View expressions | 1k-100k | Column-wide math, conditional logic |
| **Numba batch** | 100k+ | Complex per-entity logic, CPU parallelism |
| **JAX batch** | 10k+ | Pairwise interactions, ML inference, GPU |

For the Numba path, see `guide://numba`. For the JAX path, see `guide://jax`.

## View API (Batch Operations)

For 1000+ entities, View avoids Python's per-entity loop overhead. The
conditional benchmark below is 6.7x faster than Query; pure column-wide math
can reach roughly 20–25x. Measure your own workload.

```python
from pybevy.prelude import View, Mut, With
from pybevy import expr

def batch_update(
    view: View[tuple[Mut[Transform], Mut[PointLight]], With[Marker]],
    time: Res[Time],
) -> None:
    pos = view.column_mut(Transform)
    light = view.column_mut(PointLight)

    t = time.elapsed_secs()
    pos.translation.y = expr.sin(t + pos.translation.x * 0.1) * 5.0
    light.intensity = 500.0 + expr.cos(t) * 200.0
```

Key View rules:
- `column_mut(T)` for mutable, `column(T)` for read-only
- Read-only `column(T)` rejects writes: `col.a = value` and `col.a.set(value)`
  raise a `RuntimeError` pointing at `column_mut()`
- `column(T)`/`column_mut(T)` return a lazy expression column.
  For a concrete `ViewColumn` buffer use `iter_batches()` then
  `batch.column(T)`/`batch.column_mut(T)` (see `guide://numba` and `guide://jax`).
- Expressions operate on ALL matching entities at once (SIMD-like)
- Cross-component expressions supported
- `from pybevy import expr` for math functions: `sin`, `cos`, `sqrt`, `clamp`, etc.
- Numeric expressions follow Python array conventions: `%` uses the divisor's
  sign, `round()` uses ties-to-even, and `min()`/`max()` propagate NaN.
- `clamp(min, max)` propagates NaN and returns `max` when the bounds are
  reversed. `fract()` is the signed fractional part (`x - trunc(x)`).
- Expressions nest at most 32 levels deep; a deeper tree raises `ValueError`.
  Assign an intermediate result to a column and build on that column instead.

### View API - Conditional Logic

The View API supports **per-entity conditionals** via `.where()`, making it suitable for collision response and other logic that branches per entity. For large homogeneous workloads, benchmark View against Query and prefer it when the work can stay in column expressions.

`FieldExpr` (pybevy.expr or prelude) supports:
- `.where(true_val, false_val)` - vectorized ternary (like `np.where`)
- `.min(val)` / `.max(val)` / `.clamp(min, max)` - per-element clamping
- Use `&`, `|`, `~` and `.where()`; Python truth testing and chained comparisons
  raise `ValueError`.
- `<`, `>`, `<=`, `>=`, `==`, `!=` - comparisons that produce boolean columns

**Example: 5,000 bouncing balls (6.7x faster than Query)**

```python
from dataclasses import dataclass, field

@component
@dataclass
class Velocity(Component):
    vel: Vec3 = field(default_factory=lambda: Vec3.ZERO)

def bounce(
    view: View[tuple[Mut[Transform], Mut[Velocity]], With[Ball]],
    time: Res[Time],
) -> None:
    dt = time.delta_secs()
    if dt <= 0.0 or dt > 0.1:
        return

    pos = view.column_mut(Transform)
    vel = view.column_mut(Velocity)

    # Gravity + integrate (column-wide, no loop)
    vel.vel.y += GRAVITY * dt
    pos.translation.x += vel.vel.x * dt
    pos.translation.y += vel.vel.y * dt
    pos.translation.z += vel.vel.z * dt

    # Floor bounce - .where() for conditional velocity reflection
    hit_floor = pos.translation.y < BALL_R
    vel.vel.y = hit_floor.where(-vel.vel.y * RESTITUTION, vel.vel.y)
    pos.translation.y = pos.translation.y.max(BALL_R)

    # Wall bounce - combine conditions with |
    hit_xn = pos.translation.x < -WALL_LIMIT
    hit_xp = pos.translation.x > WALL_LIMIT
    vel.vel.x = (hit_xn | hit_xp).where(-vel.vel.x * RESTITUTION, vel.vel.x)
    pos.translation.x = pos.translation.x.clamp(-WALL_LIMIT, WALL_LIMIT)
```

Benchmark (5,000 balls): Query loop = **13.65ms/frame**, View API = **2.03ms/frame**.

## Visibility Optimization

Use `Visibility.Hidden` to cull entities without despawning:
```
set_component {"entity": 42, "component": "Visibility", "fields": {"variant": "Hidden"}}
```

## Shadow Casters

Every shadow-casting light re-renders all casters into its shadow map (6 cubemap
faces for point lights). Two rules:

- Tag particles and small glow meshes with `NotShadowCaster` - thousands of tiny
  casters in shadow passes is the most common silent FPS killer:
  ```python
  from pybevy.light import NotShadowCaster
  commands.spawn(Mesh3d(mesh), MeshMaterial3d(mat), NotShadowCaster(), ...)
  ```
- Budget shadows: one shadowed key light per scene, `shadow_maps_enabled=False` on
  the rest. `unlit=True` skips PBR shading, but it also discards `emissive`, so use it
  only for particles whose colour comes from `base_color` (see `guide://lighting`).

## Monitoring

```
get_performance
→ FPS, CPU/GPU/RAM usage, entity/asset counts, system profiling times
```

Use before and after changes to catch regressions.

For reload memory growth, compare completed reloads, entity/asset/system counts,
CPU allocations and per-process GPU memory after warm-up. RSS alone cannot
distinguish a leak from retained allocator pages.

`uptime_secs` is the operating-system process uptime and remains monotonic
across full reloads. `generation_uptime_secs` is Bevy real time for the current
app generation and resets on a full reload.
