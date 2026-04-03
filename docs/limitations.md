# PyBevy Limitations

Known limitations arising from the interaction between Rust's type system, Bevy's architecture, Python bindings, and the batch execution model.

## Bevy API Coverage

PyBevy does not wrap 100% of Bevy's public API. Coverage varies by module — core areas like transforms, ECS, lighting, and cameras have extensive support, while newer or more specialized subsystems may have partial or no coverage. Use `cargo run -p pybevy_lint -- coverage --bevy-path <BEVY_SRC>` to check current coverage by module.

## Rust Plugin Extensibility

Adding third-party Bevy plugins written in Rust (e.g., custom physics engines, procedural generation libraries) currently requires compiling them into the PyBevy package itself. There is no mechanism for users to load external Rust plugins at runtime — all Rust-side functionality must be baked into the repository and shipped as part of the pip package. This makes it impractical for end users to bring their own Rust plugins without forking the project.

## Batch Execution

### Expression-Only Bytecode VM

The bytecode VM (View API) supports arithmetic expressions, transcendental functions, and branchless conditional masking (`.where()`), but **not** general control flow (loops, data-dependent branching). Complex game logic requires either per-entity Python iteration (Query) or Numba JIT kernels.

### Wrapper Storage Memory Overhead

Custom components using wrapper storage are packed into power-of-two size classes (8, 16, 32, 64, 128, 256, 512, 1024 bytes). A component requiring 33 bytes is stored in a 64-byte slot, wasting up to 48% of allocated memory. At scale (e.g., 1M entities with a 33-byte component), this amounts to ~30 MB of unused padding. Components exceeding 1024 bytes cannot use wrapper storage at all and fall back to PyObject storage.

## Custom Component Storage Modes

Custom Python components have two storage modes with different trade-offs:

| Feature | Wrapper (`@component`) | PyObject (`@component(storage="python")`) |
|---------|------------------------|-------------------------------------------|
| Field types | `int`, `float`, `bool`, `Vec3`, `Vec2` | Any Python type (`str`, `list`, `dict`, classes, ...) |
| Query API | Yes | Yes |
| View API (batch) | Yes | No — raises `RuntimeError` |
| Numba JIT | Yes | No |
| `Changed[T]` | Yes | No — mutations are invisible to Bevy's change tracker |
| `Added[T]` | Yes | Yes |
| Max size | 1024 bytes | Unlimited |

By default, `@component` attempts wrapper storage and raises a `TypeError` if unsupported field types are found:

```
TypeError: Component 'Inventory' has non-primitive fields: 'items' (list).
Non-primitive fields require PyObject storage, which disables View batch
execution and Numba JIT. Use @component(storage="python") to opt in.
```

Opt in explicitly with `@component(storage="python")`:

```python
@component(storage="python")
@dataclass
class Inventory(Component):
    items: list[str]
    gold: int
```

These components work normally with `Query[Inventory]` and `Query[Mut[Inventory]]` — only the batch/JIT paths and change detection are affected.

## Change Detection

### Eager Marking on Mutable Access

Wrapper storage components support Bevy's `Changed<T>` filter via `__setattr__` hooks that call `mark_component_changed()` through a thread-local entity context. However, `Changed<T>` fires for any entity accessed through `Query[Mut[T]]`, regardless of whether fields were actually modified — matching Bevy's own semantics for `&mut T`. Use `Query[T]` (read-only) when you don't intend to mutate.

PyObject storage components do **not** support `Changed[T]` — field mutations modify the Python object directly without notifying Bevy's change tracker. `Added[T]` still works for detecting newly spawned entities.

## GIL Serialization

On standard CPython, all Python systems serialize under the GIL despite Bevy's parallel scheduler. Only free-threaded CPython (3.14t+) achieves real parallel execution of non-conflicting Python systems. The View API's batch execution and Numba JIT kernels release the GIL during computation, providing parallelism even on standard CPython.

## View API Field Type Restrictions

The View API (batch expressions) supports `f32`, `u32`, `bool`, `Vec2`, `Vec3`, `Vec4`, and `Quat` fields. **`Color` fields are not supported** in View expressions because `Color` is a Rust enum with a discriminant byte, making zero-copy column access impossible — the memory layout varies by color space variant. `Color` is supported in `from_numpy()` batch spawning (which replaces the entire field) but cannot be read or written through the View API.

Workaround: use individual float fields or compute colors in Python via `Query[Mut[PointLight]]` iteration.

## View API Filters

The View API supports `With[T]`, `Without[T]`, `Changed[T]`, and `Added[T]` filters but does not support:

- **`Entity` as a View column** — `Entity` can be included in the View type signature (e.g., `View[Entity, Mut[Transform]]`) without error, but entity IDs are accessed via `batch.entities()` rather than as a columnar expression operand. Use `batch.entities()` to get entity IDs in the same order as column data.
- **`Optional[T]`** — Optional components require per-entity presence checks. Use `Query[tuple[T, Optional[U]]]` instead.

## Free-Threaded Python

The ValidityFlag architecture uses atomic operations (`Arc<AtomicU8>`) independent of the GIL, and Bevy's scheduler-level access guarantees provide the necessary synchronisation. The extension declares `gil_used = false`, so the free-threaded interpreter does not re-enable the GIL at import. Initial validation on CPython 3.14t passes 3,533/3,534 tests and confirms genuine parallel execution of non-conflicting CPU-bound Python systems. Remaining work: broader platform testing and performance characterisation of View batch workloads under per-object mutex overhead.

## Batch Spawning

> **Note:** The batch spawning API is subject to change. The current design has known ergonomic issues that may be addressed in future releases.

### Requires World Access for Batch Components

`spawn_batch()` with NumPy batch components (from `from_numpy`) only works from `World.commands()`. Calling it from a system's `Commands` parameter raises `RuntimeError`. Systems that need batch spawning must take `World` instead of `Commands`. The legacy iterable path works from both.

### Limited Batch Component Types

Most components with `view_fields` support `from_numpy()` for per-entity batch data. Components without `from_numpy()` are passed as "uniform" (cloned identically to every spawned entity). Custom Python `@component` classes with wrapper storage also support `from_numpy()`.

`from_numpy()` field names match the component's actual field names (e.g., `Transform.from_numpy(translation=..., rotation=..., scale=...)`, `PointLight.from_numpy(intensity=...)`). Arrays are 1D or 2D NumPy arrays, auto-cast to float32.

## Component Not Invalidated After Spawn/Insert

When a component is passed to `commands.spawn()` or `entity.insert()`, the Rust data is cloned into ECS storage but the Python object remains valid. Any field borrows obtained before spawning continue to work, but modifications to the Python object no longer affect the spawned ECS entity.

```python
def setup(commands: Commands) -> None:
    bloom = Bloom()
    prefilter = bloom.prefilter

    commands.spawn(bloom)

    # prefilter is still usable but modifying it has no effect on the entity
    prefilter.threshold = 99.0  # does NOT update the spawned entity
```

This is inherent to PyO3's binding model — Python has no move semantics, so `spawn()` must clone. Invalidating the source would also break the valid pattern of reusing a component object for multiple spawns.

## Zero-Copy Mesh Context Managers — Dangling Pointer Risk

The zero-copy mesh API (`mesh.positions()`, `mesh.attribute(...)`, and their `_mut` variants) returns a NumPy array that points directly into Rust-owned `Vec` memory. The context manager's `__exit__` performs no cleanup — it does not invalidate the array. This creates several safety hazards:

**Use-after-context-exit.** If the user copies the numpy reference outside the `with` block, the array remains accessible but its pointer is not lifetime-bound to the Rust data. If the mesh is later dropped, or the attribute `Vec` reallocates (e.g., via `insert_attribute`, vertex count change), the numpy array becomes a dangling pointer — reads return garbage, writes corrupt memory, or the process segfaults.

```python
# UNSAFE — do not do this
with mesh.positions() as pos:
    saved = pos  # copies the numpy view reference, not the data

# saved still works by accident, but the pointer can dangle
saved[0, 0]  # undefined behavior if mesh is dropped/reallocated
```

**Mutable pointer escape.** The same issue is worse with `_mut` variants — a leaked mutable view can write to freed or reallocated memory.

**Nested mutable contexts.** Opening two `positions_mut()` contexts on the same mesh creates two mutable numpy views aliasing the same Rust memory, violating Rust's aliasing invariants. This is currently allowed and tested but is unsound.

**Root cause.** `PyArray2::borrow_from_array` receives `py.None()` as the "owner" object, so NumPy's reference counting does not prevent the Rust data from being freed. The context manager pattern is advisory only — there is no enforcement mechanism.

**Mitigation options (not yet implemented):**
- Invalidate the array in `__exit__` (e.g., set `base` to None, resize to 0, or replace data pointer with a sentinel)
- Use a flag/generation counter to detect stale views at access time
- Copy-on-exit to make escaping safe (trades zero-copy for safety)

**Safe alternative:** Use `positions_copy()` / `normals_copy()` which return owned numpy arrays with no dangling-pointer risk.

## Custom Message Type Limit

PyBevy supports a maximum of **20 custom message types** per application. Messages require `Messages<T>` with a concrete Rust type at compile time, so PyBevy pre-generates 20 wrapper types (`CustomMessage1`..`CustomMessage20`).

Workaround — combine related messages into a single type with a discriminator:

```python
@dataclass
class PlayerEvent(Message):
    event_type: str  # "damage", "heal", "level_up"
    player_id: int
    data: dict
```
