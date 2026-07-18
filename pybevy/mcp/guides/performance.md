# Performance Guide

Batch operations, View API, material caching, and strategy comparison for large entity counts.

## Asset Handle Caching

**NEVER** call `meshes.add()` or `materials.add()` inside Update systems. Each call creates a new GPU asset that persists after the entity is despawned - a memory leak that grows every cycle.

```python
# ❌ BAD - leaks assets every spawn cycle
def spawn_effect(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    mesh = meshes.add(Sphere(0.5))          # New GPU asset every time
    mat = materials.add(StandardMaterial(    # Another leak
        emissive=LinearRgba.rgb(10.0, 5.0, 20.0), unlit=True,
    ))
    commands.spawn(Mesh3d(mesh), MeshMaterial3d(mat), ...)
```

Pre-create handles in Startup and store them in a resource:

```python
@resource
@dataclass
class VfxAssets(Resource):
    bolt_mesh: int = 0
    bolt_mat: int = 0
    burst_mesh: int = 0
    burst_mat: int = 0

def setup(
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
    assets: ResMut[VfxAssets],
) -> None:
    assets.bolt_mesh = meshes.add(Cuboid(0.18, 35.0, 0.18))
    assets.bolt_mat = materials.add(StandardMaterial(
        emissive=LinearRgba.rgb(60.0, 30.0, 120.0),
        unlit=True, alpha_mode=AlphaMode.Add(),
    ))
    assets.burst_mesh = meshes.add(Sphere(0.8))
    assets.burst_mat = materials.add(StandardMaterial(
        emissive=LinearRgba.rgb(30.0, 15.0, 60.0),
        unlit=True, alpha_mode=AlphaMode.Add(),
    ))

# ✅ GOOD - reuses cached handles, zero asset growth
def spawn_effect(commands: Commands, assets: Res[VfxAssets]) -> None:
    commands.spawn(Mesh3d(assets.bolt_mesh), MeshMaterial3d(assets.bolt_mat), ...)

@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(VfxAssets())  # ← REQUIRED: resource must exist before Startup
        .add_systems(Startup, setup)
        .add_systems(Update, spawn_effect)
    )
```

**Key points:**
- `app.insert_resource(VfxAssets())` is **required** - without it, `ResMut[VfxAssets]` in Startup will error with "resource not found in world"
- Asset handles are stored as `int` (the handle ID returned by `meshes.add()`)
- Alternative: use `commands.insert_resource(VfxAssets(...))` in Startup instead of `ResMut[VfxAssets]`, but then the resource is only available in Update (not Startup)

**Applies to:** projectiles, VFX, particles, pooled enemies - anything spawned more than once.

**Verify:** `get_performance` → Assets line. Mesh/Material counts should stay flat after Startup.

## Material Mutation Caching

**NEVER** call `materials.get_mut(handle)` every frame on a shared material. This re-prepares bind groups and re-batches ALL entities using that material.

```python
# ❌ BAD - 50ms/frame for 150k entities
def animate(materials: ResMut[Assets[StandardMaterial]], handle: Res[MatHandle]):
    mat = materials.get_mut(handle.0)
    mat.base_color = Color.linear_rgb(...)  # Every frame!

# ✅ GOOD - only update on change
def animate(materials: ResMut[Assets[StandardMaterial]], handle: Res[MatHandle]):
    new_color = compute_color(time)
    if new_color != last_color:
        mat = materials.get_mut(handle.0)
        mat.base_color = new_color
```

## Entity Count Guidelines

Typical frame rates (RTX 5090, 4K):
- Up to 10k entities: No special considerations needed
- 10k–80k entities: ~80 FPS, use View API for bulk operations
- 80k–150k entities: ~43 FPS, CPU render extraction becomes bottleneck

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
        Transform.from_numpy(translation=positions),
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        PointLight(intensity=500.0),  # Uniform: cloned to all entities
    )
```

**Key points:**
- `from_numpy()` returns a `Batchable` - an opaque batch object consumed by `spawn_batch`
- Uniform components (plain instances like `PointLight(...)`) are cloned to every entity
- Works with regular system `Commands` (deferred) or `World.commands()` (immediate)
- Via `World.commands()`, returns `list[Entity]`; via system `Commands`, returns `None` (entities created at flush)
- Arrays are auto-cast to float32 and validated for shape at `from_numpy()` time
- `Transform.from_numpy()` accepts `translation` (Nx3), `rotation` (Nx4), `scale` (Nx3) - all optional
- Any Rust component with `view_fields` supports `from_numpy()` (e.g., `PointLight.from_numpy(intensity=arr)`)
- Custom `@component` classes with wrapper storage also support `from_numpy()`
- Use View API afterwards for bulk per-entity updates (see below)

**Limitations:**
- `from_numpy()` requires wrapper storage - custom `@component` classes with `storage="python"` do not support it
- All non-Batchable components are cloned uniformly to every entity. To vary materials across batched entities, call `spawn_batch` once per material with the appropriate subset of positions.

**Legacy iterable path** still works for small batches:
```python
commands.spawn_batch([(Transform.from_xyz(i, 0, 0), Name(f"e{i}")) for i in range(100)])
```

## Choosing a Batch Strategy

| Approach | Entity Count | Use Case | Typical Speed |
|----------|-------------|----------|---------------|
| Query iteration | < 1k | Method calls, simple logic | 1x baseline |
| View expressions | 1k–100k | Column-wide math, conditional logic | 30–50x |
| **Numba batch** | 100k+ | Complex per-entity logic, CPU parallelism | 50–100x |
| **JAX batch** | 10k+ | O(n²) interactions, ML inference, GPU | GPU-dependent |

For the Numba path, see `guide://numba`. For the JAX path, see `guide://jax`.

## View API (Batch Operations)

For 1000+ entities, the View API is 30–50x faster than Query:

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
- Expressions operate on ALL matching entities at once (SIMD-like)
- Cross-component expressions supported
- `from pybevy import expr` for math functions: `sin`, `cos`, `sqrt`, `clamp`, etc.
- Numeric expressions follow Python array conventions: `%` uses the divisor's
  sign, `round()` uses ties-to-even, and `min()`/`max()` propagate NaN.
- `clamp(min, max)` propagates NaN and returns `max` when the bounds are
  reversed. `fract()` is the signed fractional part (`x - trunc(x)`).

### View API - Conditional Logic

The View API supports **per-entity conditionals** via `.where()`, making it suitable for collision response and any logic that branches per entity. **Always prefer View over Query for 1000+ entities**, even when conditionals are needed.

Available conditional operators on `FieldExpr`:
- `.where(true_val, false_val)` - vectorized ternary (like `np.where`)
- `.min(val)` / `.max(val)` / `.clamp(min, max)` - per-element clamping
- `|` and `&` - combine boolean conditions
- `<`, `>`, `<=`, `>=`, `==`, `!=` - comparisons that produce boolean columns

**Example: 5,000 bouncing balls (6.7x faster than Query)**

```python
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
set_component {"entity_id": 42, "component": "Visibility", "fields": {"value": "Hidden"}}
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
  the rest. For emissive dust/particles also set `unlit=True` (skips PBR shading).

## Monitoring

```
get_performance
→ FPS, CPU/GPU/RAM usage, entity/asset counts, system profiling times
```

Use before and after changes to catch regressions.
