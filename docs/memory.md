# Memory Footprint

How PyBevy's memory usage compares to pure Rust Bevy, and where the overhead comes from.

## Summary

**ECS storage has zero overhead for native components.** When you use built-in Bevy types like `Transform`, `PointLight`, or `Camera3d`, the data in Bevy's archetype tables is byte-for-byte identical to pure Rust Bevy. PyBevy does not add any per-entity wrapper or indirection — the real Rust struct lives directly in the archetype table.

**Custom Python components have minimal storage overhead.** Wrapper storage (`@component` with primitive fields) stores data as fixed-size byte arrays in the archetype table, with at most power-of-2 rounding waste. PyObject storage adds ~50 bytes/entity for the Python heap object.

**Spawning through Python leaves no live per-entity Python objects.** The benchmarks below show ~400 B/ent RSS overhead vs pure Rust, but `tracemalloc` confirms **zero live Python allocations per entity** after spawning. The RSS overhead is entirely CPython's pymalloc holding onto freed arenas — a standard allocator behavior, not data PyBevy is keeping. Once entities are spawned, per-frame iteration via Query or View allocates no additional memory.

## Benchmark Results (100k entities)

Both sides use `App` + `MinimalPlugins` + a `Startup` system — apples-to-apples.

| Component | Rust B/ent | PyBevy B/ent | Overhead | Ratio |
|---|---:|---:|---:|---:|
| Transform (48B native) | 194 | 613 | +419 | 3.2x |
| Vec3-like (wrapper) | 77 | 463 | +386 | 6.0x |
| Vec3-like (PyObject) | 77 | 509 | +432 | 6.6x |
| Vec3+String (PyObject) | 136 | 525 | +389 | 3.9x |
| Transform + Velocity | 214 | 785 | +571 | 3.7x |
| Transform + Marker | 204 | 851 | +647 | 4.2x |

The ~400 B/ent constant reflects Python allocator retention, not ECS data overhead. The "Ratio" column looks worse for small components (like a bare Vec3 at 12 bytes in Rust) because this constant dominates.

## Where the RSS Overhead Comes From

### Spawn-time allocator retention (~400 B/ent in RSS)

The RSS overhead in the benchmarks is **not** per-entity data stored in the ECS. It is freed memory that the Python/Rust allocators haven't returned to the OS. Verified with `tracemalloc`:

```
RSS delta:                 ~610 B/ent
  Rust ECS data (same as pure Bevy):  ~194 B/ent
  Python allocator retention:         ~416 B/ent (freed, not returned to OS)
  Live Python objects per entity:       0 B/ent  (tracemalloc confirms)
```

**`gc.collect()` does not help** — there is nothing to collect. All Python temporaries created during spawning are already freed by the time `app.update()` returns. The RSS overhead persists because:

- **pymalloc arena retention** — CPython's pymalloc uses 256KB arenas with 4KB pools. Once allocated, arenas are almost never returned to the OS even when all objects within them are freed. This is standard CPython behavior, not PyBevy-specific.
- **Rust allocator behavior** — The Rust-side allocator (typically jemalloc or system malloc) also retains freed pages for reuse rather than returning them to the OS via `munmap`.

The spawn path creates temporary objects that trigger this retention:

- **PyO3 wrapper objects** — Each `commands.spawn(Transform(...))` creates temporary wrapper objects, component type lookups through the bridge registry, and Python call frames.
- **Command queue entries** — PyBevy's command queue buffers Python objects before flushing to Bevy's ECS. All entries are freed after flush.

### Component data overhead (varies by storage type)

| Storage type | ECS overhead vs Rust | Notes |
|---|---|---|
| **Native components** (Transform, PointLight, ...) | Zero | Stored identically to pure Rust Bevy |
| **Wrapper storage** (`@component` with primitives) | Power-of-2 rounding | Data stored as `ComponentWrapper{8,16,32,...}` byte arrays. A 3-field `float` component (24 bytes of f64) goes into a 32-byte wrapper. |
| **PyObject storage** (`@component(storage="python")`) | ~50 B/ent extra vs wrapper | ECS stores an 8-byte pointer; the actual Python object lives on the heap (~56+ bytes with GC header) |

### What is NOT per-entity

These costs are fixed regardless of entity count:

- **Python interpreter**: ~20-30 MB baseline RSS
- **PyO3 type registrations and bridge registry**: negligible
- **ValidityFlag**: One `Arc<AtomicU8>` per system execution context (~32 bytes), shared across all entities in that system

## Storage Architecture Details

### Native Bevy components

Components like `Transform`, `PointLight`, `Camera3d` etc. are stored directly in Bevy's archetype tables as the real Rust type. PyBevy creates temporary `PyTransform` / `PyPointLight` wrapper objects only when accessed through Query iteration. These wrappers use `ComponentStorage<T>` with a `Borrowed` variant that holds a raw pointer back into the archetype table — no data is copied.

```
ECS archetype table (identical to pure Rust Bevy):
  Entity 0: [Transform (48B)] [PointLight (64B)]
  Entity 1: [Transform (48B)] [PointLight (64B)]
  ...

During Query iteration (temporary, not stored):
  PyTransform { storage: Borrowed { ptr -> archetype[i].transform } }
```

### Wrapper storage custom components

Python `@component` classes with primitive-only fields (`int`, `float`, `bool`) are serialized into fixed-size byte arrays and stored in Bevy's archetype tables as `ComponentWrapper{8,16,32,64,128,256,512,1024}`:

```python
@component
@dataclass
class Velocity(Component):
    vx: float  # 8 bytes (f64)
    vy: float  # 8 bytes (f64)
    vz: float  # 8 bytes (f64)
# Total: 24 bytes -> stored in ComponentWrapper32 (8 bytes wasted)
```

The waste from power-of-2 rounding is at most 50% of the data size. This is a deliberate tradeoff: fixed sizes let the View API and Numba access component fields as contiguous typed arrays without per-entity dispatch.

### PyObject storage custom components

Components with non-primitive fields (`str`, `list`, `dict`, custom classes) or those explicitly marked `@component(storage="python")` store a Python object pointer in ECS:

```
ECS archetype table:
  Entity 0: [*mut PyObject (8B)]   -> Python heap: Velocity { vx, vy, vz, label }
  Entity 1: [*mut PyObject (8B)]   -> Python heap: Velocity { vx, vy, vz, label }
```

The Python heap object carries standard CPython overhead: object header (16B), GC tracking (16B), `__dict__` or dataclass slots, and the field values themselves.

## Running the Benchmarks

### Prerequisites

```bash
# Build PyBevy in release mode
poetry run maturin develop --release

# Build the Rust memory baseline binary
cargo build --example memory_baseline --release --features linux-display
```

### Run

```bash
# Full comparison table
poetry run pytest benches/ecs/test_ecs_memory.py -v -s -k summary

# All individual tests
poetry run pytest benches/ecs/test_ecs_memory.py -v -s

# Rust baseline only (no Python needed)
./target/release/examples/memory_baseline transform 100000
./target/release/examples/memory_baseline velocity_string 100000
```

### Methodology

Each measurement runs in an isolated subprocess to prevent RSS contamination from previous allocations (RSS rarely decreases after free in a running process). The benchmark:

1. Creates an `App` with a `Startup` system that spawns N entities
2. Calls `app.initialize()` to set up schedules and plugins
3. Measures RSS before/after `app.update()` (which runs the Startup system)
4. Reports `(rss_after - rss_before) / N` as bytes-per-entity

The Rust baseline (`benches/ecs/memory_baseline.rs`) uses the same pattern: `App` + `MinimalPlugins` + `Startup` system + `app.update()`.

## Practical Implications

- **For typical game scenes** (hundreds to low thousands of entities): memory overhead is negligible. The ~400 B/ent overhead at 1,000 entities is 400 KB — invisible next to textures, meshes, and audio.
- **For large simulations** (100k+ entities): the overhead becomes measurable. 100k entities costs ~40 MB extra vs pure Rust. Consider using batch spawning (`spawn_batch` with NumPy arrays) which bypasses the per-entity Python object creation.
- **Component data itself** is efficient: native components have zero overhead, wrapper storage wastes at most a few bytes per entity from rounding, and PyObject storage adds ~50 B/ent for the Python heap object.
- **The overhead is spawn-time, not query-time**: once entities exist in the ECS, iterating them via Query or View has no additional memory allocation per frame.
