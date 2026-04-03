# Safety in PyBevy

This document describes how PyBevy maintains memory safety when bridging Python's dynamic nature with Rust's strict safety guarantees.

## Overview

PyBevy implements multiple layers of safety mechanisms to prevent common memory safety bugs while maintaining acceptable performance:

1. **Runtime Validity Checking** - Prevents use-after-free bugs
2. **Parameter Validation** - Enforces Rust's borrowing rules (between parameters)
3. **Python Reference Semantics** - Standard Python behavior for references within parameters
4. **Free-Threading Mode** - Works with Python 3.13+ without GIL (mostly safe)
5. **Async Detection** - Actively blocks async systems that would break safety
6. **Borrowed Field Access** - Enables safe direct mutations
7. **Type Safety** - Leverages Python's type hints and runtime checks

---

## 1. Runtime Validity Checking

**Purpose**: Prevent use-after-free bugs when accessing ECS data after a system completes

### The Problem

In Bevy, system parameters like `Query`, `Commands`, and `ResMut` are only valid during system execution. If Python code stores a reference and tries to use it later, it would access freed memory:

```python
stored_query = None

def bad_system(query: Query[Transform]) -> None:
    global stored_query
    stored_query = query  # Dangerous!

# After system completes...
stored_query[0]  # Would be use-after-free without protection!
```

### The Solution: ValidityFlag with RAII Guards

PyBevy uses **runtime validity flags** with RAII guards to automatically invalidate all system parameters when execution completes:

```rust
// In DynamicSystem::run_unsafe() - src/dynamic_system.rs
let validity = ValidityFlag::new();
let _guard = ValidityGuard::new(validity.clone());

// All parameters share the same validity flag
let py_world = unsafe { PyWorld::new(world_mut, validity.clone()) };
let py_commands = unsafe { PyCommands::new(&mut commands, validity.clone()) };
let py_query = PyQuery::new(query_state, validity.clone());

// Execute Python system...
python_function.call()?;

// When scope ends, guard drops → validity flag set to false
// All parameters become invalid, even on panic!
```

### Runtime Checks on Every Access

Every method that accesses ECS data checks the validity flag first:

```rust
impl PyWorld {
    pub fn spawn_empty(&self) -> PyResult<PyEntityCommands> {
        self.check_valid()?;  // Raises RuntimeError if accessed after system
        // Safe to use world pointer
        let entity = unsafe { self.world_mut() }.spawn_empty();
        Ok(PyEntityCommands::new(entity, self.validity.clone()))
    }
}
```

**Result**: Attempting to use system parameters after the system completes raises a clear error instead of causing undefined behavior:

```python
def system(query: Query[Transform]) -> None:
    global stored_query
    stored_query = query

# Later...
stored_query[0]  # RuntimeError: Component access is no longer valid
```

### Performance

- **Overhead**: ~4ns per validity check (atomic load with Acquire ordering)
- **Impact**: ~4-8% of Query iteration cost (negligible compared to Python overhead)
- **Benefit**: Complete prevention of use-after-free bugs

---

## 2. Parameter Validation (Borrowing Rules)

**Purpose**: Enforce Rust's borrowing rules at system registration time for **all** parameter types

### The Problem

Rust's borrow checker prevents multiple mutable references or mixing mutable and immutable references to the same data:

```rust
// This won't compile in Rust:
fn bad_system(
    query1: Query<&mut Transform>,  // Mutable borrow
    query2: Query<&mut Transform>,  // Second mutable borrow - ERROR!
) { }
```

Python has no compile-time checks, so PyBevy must detect these conflicts at runtime.

### The Solution: Comprehensive Registration-Time Validation

PyBevy validates **all** system parameters when the system is **added** to the app (before it runs). This includes:

1. **Component Access** (Query/View parameters)
2. **Resource Access** (Res/ResMut parameters)
3. **Asset Access** (Assets[T] parameters)
4. **World Exclusivity** (World parameter)

**Implementation**: `src/ecs/dynamic_system.rs:1142-1209` (`validate_parameters()`)

The validation tracks three separate access maps:
- `component_access` - Query/View component conflicts
- `resource_access` - Res/ResMut resource conflicts
- `asset_access` - Assets[T] asset type conflicts

Plus special handling for World's exclusive access requirement.

**Called from**: `DynamicSystem::initialize()` when system is added to the app

### Validation Coverage

| Parameter Type | Validates | Test Coverage |
|----------------|-----------|---------------|
| **Query** | Component access conflicts | 10+ tests |
| **View** | Component access conflicts | 8 tests |
| **Resource** | Resource borrow conflicts | 9 tests |
| **Assets** | Asset type borrow conflicts | 8 tests |
| **Time** | Treated as resource conflict | 5 tests |
| **World** | Exclusive access enforcement | 14 tests |
| Commands | No validation (buffers only) | - |
| Local | No validation (isolated state) | - |
| AssetServer | No validation (read-only) | - |

**Total**: 58 safety tests (100% passing)

### Examples: Query Conflicts

**Rejected - Multiple mutable queries**:
```python
def bad_system(
    query1: Query[Mut[Transform]],
    query2: Query[Mut[Transform]]  # ERROR at registration!
) -> None:
    pass

app.add_systems(Update, bad_system)
# RuntimeError: System 'bad_system' has conflicting component access:
#  - Parameter 1 requests mutable access to Transform
#  - Parameter 0 already has mutable access to Transform
```

**Rejected - Read-write conflict**:
```python
def bad_system(
    read_query: Query[Transform],      # Immutable
    write_query: Query[Mut[Transform]]  # Mutable - ERROR!
) -> None:
    pass
```

**Allowed - Multiple read-only queries**:
```python
def good_system(
    query1: Query[Transform],  # Immutable
    query2: Query[Transform]   # Immutable - OK!
) -> None:
    pass  # Multiple readers allowed
```

**Allowed - Different components**:
```python
def good_system(
    transforms: Query[Mut[Transform]],  # Mutable Transform
    lights: Query[Mut[PointLight]]      # Mutable PointLight - OK!
) -> None:
    pass  # Different components don't conflict
```

### Examples: View Conflicts

View parameters follow the **same borrowing rules** as Query:

**Rejected - View + Query conflict**:
```python
def bad_system(
    view: View[Mut[Transform]],   # Mutable
    query: Query[Transform]        # Immutable - ERROR!
) -> None:
    pass

# RuntimeError: conflicting component access to Transform
```

**Rejected - Double mutable View**:
```python
def bad_system(
    view1: View[Mut[Transform]],
    view2: View[Mut[Transform]]  # ERROR!
) -> None:
    pass
```

**Allowed - Multiple read-only Views**:
```python
def good_system(
    view1: View[Transform],
    view2: View[Transform]  # OK!
) -> None:
    pass  # Multiple readers allowed
```

**Workaround - Split into separate systems**:
```python
# Instead of mixing View + Query in one system:
def modify_system(view: View[Mut[Transform]]) -> None:
    # Modify with View in Update
    pass

def verify_system(query: Query[Transform]) -> None:
    # Verify with Query in Last
    pass

app.add_systems(Update, modify_system)
app.add_systems(Last, verify_system)
```

### Examples: Resource Conflicts

**Rejected - Double mutable resources**:
```python
def bad_system(
    res1: ResMut[GameSettings],
    res2: ResMut[GameSettings]  # ERROR!
) -> None:
    pass

# RuntimeError: conflicting resource access to GameSettings
```

**Rejected - Resource read-write conflict**:
```python
def bad_system(
    res: Res[Time],
    res_mut: ResMut[Time]  # ERROR!
) -> None:
    pass
```

**Allowed - Multiple read-only resources**:
```python
def good_system(
    res1: Res[GameSettings],
    res2: Res[GameSettings]  # OK!
) -> None:
    pass  # Multiple readers allowed
```

**Allowed - Different resources**:
```python
def good_system(
    settings: ResMut[GameSettings],
    time: ResMut[Time]  # OK - different resources!
) -> None:
    pass
```

### Examples: Assets Conflicts

**Rejected - Double mutable assets**:
```python
def bad_system(
    mats1: ResMut[Assets[StandardMaterial]],
    mats2: ResMut[Assets[StandardMaterial]]  # ERROR!
) -> None:
    pass

# RuntimeError: conflicting asset access to Assets[StandardMaterial]
```

**Rejected - Assets read-write conflict**:
```python
def bad_system(
    images: Res[Assets[Image]],
    images_mut: ResMut[Assets[Image]]  # ERROR!
) -> None:
    pass
```

**Allowed - Different asset types**:
```python
def good_system(
    materials: ResMut[Assets[StandardMaterial]],
    images: ResMut[Assets[Image]]  # OK - different types!
) -> None:
    pass
```

### Examples: World Exclusive Access

World requires **EXCLUSIVE** access to the entire ECS. It conflicts with any parameter that accesses world state.

**Rejected - World + Query**:
```python
def bad_system(
    world: World,
    query: Query[Transform]  # ERROR!
) -> None:
    pass

# RuntimeError: World parameter requires EXCLUSIVE access to entire ECS
#  - Parameter 1 conflicts with World (type: Query)
#  World can only be used with Commands and Local parameters.
```

**Rejected - World + Resources**:
```python
def bad_system(
    world: World,
    time: Res[Time]  # ERROR!
) -> None:
    pass
```

**Rejected - World + Assets**:
```python
def bad_system(
    world: World,
    assets: ResMut[Assets[Mesh]]  # ERROR!
) -> None:
    pass
```

**Allowed - World + Commands**:
```python
def good_system(
    world: World,
    commands: Commands  # OK - Commands just buffers!
) -> None:
    pass  # Commands doesn't access world state during system
```

**Allowed - World alone**:
```python
def good_system(world: World) -> None:
    # Can use world.spawn(), world.query(), world.resource(), etc.
    pass
```

---

## 3. Python Reference Semantics vs Rust Borrowing

**Purpose**: Clarify differences between Rust's compile-time guarantees and Python's runtime behavior

### The Difference

Parameter validation (Section 2) prevents conflicts **between** parameters, which matches Rust's borrowing rules. However, **within** a single parameter, PyBevy follows normal Python reference semantics:

```python
def system(query: Query[Mut[Transform]]) -> None:
    transforms = list(query)  # Convert to Python list

    # Multiple references to the same entity - normal Python!
    ref1 = transforms[0]
    ref2 = transforms[0]

    # Both reference the same object (standard Python behavior)
    ref1.translation.x = 100.0
    print(ref2.translation.x)  # Prints 100.0 - same object

    ref2.translation.x = 200.0
    print(ref1.translation.x)  # Prints 200.0 - same object
```

**This is normal Python behavior**:
```python
# Regular Python works exactly the same way:
class Point:
    def __init__(self, x):
        self.x = x

my_list = [Point(10), Point(20)]
ref1 = my_list[0]
ref2 = my_list[0]  # Same reference, not a copy

ref1.x = 999
print(ref2.x)  # Prints 999 - this is how Python works!
```

**Why this is NOT a safety hole**:
- Standard Python reference semantics (expected behavior)
- No memory corruption, data races, or undefined behavior
- ValidityFlag still prevents use-after-free
- Python GIL prevents concurrent access issues
- Easy to debug (wrong values are obvious, not silent corruption)

### Behavioral Differences from Rust Bevy

**What Rust prevents (compile-time)**:
- Multiple mutable borrows **between system parameters**
- Rust: `fn system(q1: Query<&mut T>, q2: Query<&mut T>)` Won't compile
- PyBevy: Prevented by parameter validation (Section 2)

**What Rust prevents (compile-time) but Python allows (runtime)**:
- Multiple references to same object **within a parameter**
- Rust: Borrow checker tracks individual entity borrows
- Python: Reference semantics allow multiple refs to same object

**The only semi-realistic edge case**:

```python
def system(query: Query[Mut[Transform]]) -> None:
    entities = list(query)

    # Nested iteration - might process same entity twice
    for e1 in entities:
        for e2 in entities:
            if distance(e1, e2) < 10.0:
                apply_force(e1, e2)  # Double-applies if e1 == e2
```

**But this is just normal Python**:
```python
# Same issue exists in any Python code:
points = [Point(1, 2), Point(3, 4)]
for p1 in points:
    for p2 in points:
        # p1 and p2 might be the same - check if needed
        if p1 is not p2:  # Standard Python pattern
            process(p1, p2)
```

### Test Coverage

Three tests in `tests/safety/test_intra_query_aliasing.py` document this behavior:

1. **`test_aliasing_via_list()`** - Multiple refs to same entity work (normal Python)
2. **`test_aliasing_during_iteration()`** - Stored refs see modifications (expected)
3. **`test_double_modification_through_aliases()`** - Both refs affect same object (correct)

These tests **PASS** to document that this follows standard Python reference semantics.

### Why PyBevy Doesn't Add Runtime Checks

**Decision**: Use standard Python reference semantics (no additional tracking)

**Reasoning**:
- This is how Python works - not a bug to fix
- Adding runtime tracking would add overhead to ALL queries for a non-issue
- Python developers expect reference semantics
- No actual safety violation (no memory corruption, data races, or UB)
- Easy to reason about and debug

### Common Usage Patterns

**Standard iteration (most common)**:
```python
def system(query: Query[Mut[Transform]]) -> None:
    for transform in query:
        transform.translation.x += 1.0  # Standard pattern
```

**List conversion for random access**:
```python
def system(query: Query[Mut[Transform]]) -> None:
    transforms = list(query)

    for transform in transforms:
        transform.translation.x += 1.0  # Works fine
```

**Nested iteration (use identity check if needed)**:
```python
def system(query: Query[tuple[Mut[Transform], Entity]]) -> None:
    entities = [(t, e) for t, e in query]

    for t1, e1 in entities:
        for t2, e2 in entities:
            if e1 != e2:  # Standard Python pattern - check identity
                process(t1, t2)
```

**Using `is` operator for identity checks**:
```python
def system(query: Query[Mut[Transform]]) -> None:
    transforms = list(query)

    for t1 in transforms:
        for t2 in transforms:
            if t1 is not t2:  # Python identity check
                # Different entities
                process(t1, t2)
```

---

## 4. Free-Threading Mode (Python 3.13+)

**Purpose**: Explain PyBevy's behavior in Python's free-threaded mode (no GIL)

### Overview

Python 3.13+ introduced optional free-threaded mode where the Global Interpreter Lock (GIL) is disabled, allowing true parallel execution of Python code. PyBevy declares `gil_used = false` on the extension module so the free-threaded interpreter does not re-enable the GIL at import. Initial validation on CPython 3.14t passes 3,533/3,534 tests and confirms genuine parallel execution of non-conflicting CPU-bound Python systems. PyBevy's core safety mechanisms remain effective in this mode, with some additional considerations.

**Risk Level**: 15/100 (Low-Medium)

Most of PyBevy's safety comes from **Bevy's ECS scheduler** and **parameter validation**, not the GIL. The GIL was defensive redundancy.

### What Remains Safe

**All core PyBevy safety mechanisms work without GIL**:

1. **ValidityFlag** - Uses `Arc<AtomicU8>` (already thread-safe, tracks Read/Write/Invalid)
   ```rust
   pub struct ValidityFlag(Arc<AtomicU8>);  // Atomic operations
   ```

2. **Parameter validation** - Prevents concurrent access at registration time
   ```python
   # These conflicts are REJECTED regardless of GIL:
   def bad(q1: Query[Mut[Transform]], q2: Query[Mut[Transform]]) -> None:
       pass  # RuntimeError - conflicting access
   ```

3. **Bevy's scheduler** - Ensures systems with conflicting access never run in parallel
   - `Query[Mut[T]]` + `Query[Mut[T]]` → Never concurrent
   - `ResMut[R]` + `ResMut[R]` → Rejected by validation
   - Each entity accessed by at most one system at a time

4. **Wrapper storage components** - Mutations go through Rust memory
   ```python
   @component
   class Health(Component):
       value: float  # Stored as Rust f64 - thread-safe

   def system(query: Query[Mut[Health]]) -> None:
       for h in query:
           h.value += 1.0  # Safe - writes to Rust memory
   ```

5. **Borrowed field access** - Protected by Bevy's exclusive access
   ```python
   def system(query: Query[Mut[Transform]]) -> None:
       for t in query:
           t.translation.x = 100.0  # Safe - Bevy prevents conflicts
   ```

### What Requires Caution

**CAUTION:Global Python state (User responsibility)**:

```python
# Global state - NOT protected by PyBevy!
scores = []

def system1(query: Query[Transform]) -> None:
    scores.append(1)  # Thread 1

def system2(query: Query[Transform]) -> None:
    scores.append(2)  # Thread 2 - DATA RACE without GIL!
```

**Why it's unsafe**:
- Python lists/dicts are NOT thread-safe without GIL
- PyBevy cannot detect or prevent global state access
- This is standard Python threading responsibility

**Workaround - Use Local parameters**:
```python
from dataclasses import dataclass, field
from pybevy.ecs import Local

@dataclass
class Scores:
    values: list = field(default_factory=list)

def system(scores: Local[Scores]) -> None:
    scores.value.values.append(1)  # Each system instance gets its own list
```

**CAUTION:Custom resource complex fields (Usually safe)**:

```python
@resource
class GameState(Resource):
    enemies: list[Enemy]  # Python list

def system(state: ResMut[GameState]) -> None:
    state.enemies.append(enemy)  # Usually safe
```

**Why it's usually safe**:
- Parameter validation prevents two systems from getting `ResMut[GameState]`
- Only ONE system can mutate GameState at a time
- Bevy's scheduler enforces exclusive access

**Edge case - Still unsafe**:
```python
# If GameState is accessed from OUTSIDE systems (direct Python code)
# then it's not protected by Bevy's scheduler
```

**CAUTION: Spawning threads inside systems**:

Passing component references to threads spawned within a system bypasses PyBevy's safety guarantees. Without the GIL, two threads calling `as_mut()` on the same component simultaneously would create a data race on the underlying Rust memory. In practice the effect is limited to corrupted field values (torn writes on non-atomic types). A thread-id check in the validity flag could detect this, but would add a branch to every component access on the hot path.

### Safe Patterns

**Standard component mutations**:
```python
def system(query: Query[Mut[Transform]]) -> None:
    for transform in query:
        transform.translation.x += 1.0  # Safe - Bevy prevents conflicts
```

**Resource access with parameter validation**:
```python
def system(time: Res[Time], settings: ResMut[GameSettings]) -> None:
    if time.elapsed_secs() > 10.0:
        settings.difficulty = "hard"  # Safe - exclusive access
```

**Primitive fields in custom components**:
```python
@component
class Player(Component):
    health: float  # Stored efficiently
    score: int

def system(query: Query[Mut[Player]]) -> None:
    for player in query:
        player.health -= 1.0  # Safe
```

**Using Local for thread-local state**:
```python
from dataclasses import dataclass
from pybevy.ecs import Local

@dataclass
class Counter:
    count: int = 0

def system(counter: Local[Counter], query: Query[Transform]) -> None:
    # Each system instance gets its own Counter
    counter.value.count += len(list(query))
```

### Explicitly Prevented: Async Systems

**PyBevy actively blocks async systems at registration time:**

```python
import asyncio

async def async_system(query: Query[Mut[Transform]]) -> None:
    transforms = list(query)
    await asyncio.sleep(0.1)  # Would suspend execution
    transforms[0].translation.x = 100.0

# This raises RuntimeError at registration:
app.add_systems(Update, async_system)
# RuntimeError: Async systems (async def) are not supported.
# Use synchronous 'def' instead of 'async def'.
# Async functions would break PyBevy's safety guarantees
# (ValidityFlag becomes invalid when function suspends).
```

**Why async is dangerous:**
- ValidityGuard uses RAII - drops when async function suspends at `await`
- When function resumes, ValidityFlag is invalid but execution continues
- Parameters point to freed/moved memory → use-after-free
- Would completely bypass PyBevy's safety architecture

**Detection:**
- Checked at system registration using `inspect.iscoroutinefunction()`
- Zero runtime overhead (one-time check)
- Clear error message explains the safety issue

**Alternative for async operations:**
```python
# Instead of async systems, use regular systems with async libraries
def system(commands: Commands) -> None:
    # Start async task in background
    asyncio.create_task(load_assets())
    # System completes immediately - ValidityFlag safe

# Or use callbacks
def system(commands: Commands, loader: ResMut[AssetLoader]) -> None:
    loader.queue_load("texture.png", on_complete=lambda tex: ...)
```

### Unsafe Patterns (Free-Threaded Mode)

**UNSAFE:Global mutable state**:
```python
cache = {}  # Global - NOT thread-safe!

def system(query: Query[Entity]) -> None:
    cache[entity.id()] = data  # DATA RACE!
```

**UNSAFE:Shared closures capturing mutable state**:
```python
shared_list = []

def make_system():
    def system(query: Query[Transform]) -> None:
        shared_list.append(1)  # Captures shared_list - unsafe!
    return system
```

**UNSAFE:Accessing resources outside Bevy systems**:
```python
@resource
class GameState(Resource):
    data: list[int]

game_state = GameState()
app.insert_resource(game_state)

# Later, outside any system:
game_state.data.append(1)  # NOT protected by Bevy scheduler!
```

### PyO3 Thread Safety

**PyO3's behavior** (free-threaded mode):
- `Python::attach()` does NOT acquire GIL (multiple can run concurrently)
- `Py<T>` is `Send + Sync` - can be sent between threads
- **Users** must handle synchronization for shared Python objects

**PyBevy's approach**:
- Relies on Bevy's scheduler to prevent concurrent access
- Parameter validation enforces exclusive access at registration time
- ValidityFlag uses atomics (thread-safe regardless of PyO3 behavior)

### Detection

PyBevy automatically detects free-threaded mode:

```rust
fn detect_gil_status() -> bool {
    Python::attach(|py| {
        if let Ok(sys) = py.import("sys") {
            if let Ok(enabled) = sys.getattr("_is_gil_enabled") {
                return enabled.call0().extract().unwrap_or(true);
            }
        }
        true  // GIL enabled by default
    })
}
```

Used for diagnostics/logging. The extension module declares `gil_used = false` via `#[pymodule(gil_used = false)]`, so the free-threaded interpreter does not re-enable the GIL at import time.

### Thread Safety Summary

| Component | GIL Mode | Free-Threaded | Notes |
|-----------|----------|---------------|-------|
| ValidityFlag | Safe | Safe | Uses atomics |
| Parameter validation | Safe | Safe | Registration-time |
| Wrapper components | Safe | Safe | Rust storage |
| PyObject components | Safe | Safe | Bevy prevents conflicts |
| Built-in resources | Safe | Safe | Exclusive access enforced |
| Custom resources | Safe | Safe* | *If only accessed in systems |
| Global Python state | Discouraged | Unsafe | User responsibility |
| Borrowed fields | Safe | Safe | Bevy prevents conflicts |

### Recommendations

**For most users**: Free-threading works fine with PyBevy. Follow normal ECS patterns:
- DO: Access data through system parameters
- DO: Use Local for thread-local state
- DON'T: Use global Python mutable state
- DON'T: Access resources outside systems

**Advanced users**: If you need shared Python state:
1. Use `threading.Lock` explicitly
2. Store state in Bevy resources (accessed via parameters)
3. Use message passing instead of shared mutable state

### Why PyBevy Is Mostly GIL-Independent

PyBevy's safety architecture was designed around **Bevy's guarantees**, not Python's:

1. **Bevy's scheduler** prevents concurrent mutable access to components/resources
2. **Parameter validation** enforces borrowing rules at registration time
3. **ValidityFlag** uses atomics (works without GIL)
4. **Borrowed fields** point to Bevy-managed memory (not Python objects)

The GIL was **defensive redundancy** - an extra layer of protection against user code doing unsafe things (global state). The core safety mechanisms work regardless of GIL status.

---

## 5. Borrowed Field Access

**Purpose**: Enable direct field mutations that persist to ECS memory

### The Problem

Python's property access model creates copies:

```python
# This DOESN'T work without borrowed field access:
for transform in Query[Mut[Transform]]:
    transform.translation.x = 100.0  # Modifies a COPY
    # Original ECS data unchanged!
```

The issue: `transform.translation` returns a copy of the Vec3. Modifying `copy.x` doesn't affect the ECS.

### The Solution: Borrowed Field Pattern

PyBevy implements an enum-based storage pattern where property getters return **borrowed** references pointing directly to component fields:

```rust
// Vec3 can be either owned or borrowed - src/math/vec3.rs
enum Vec3Storage {
    Owned(Vec3),  // Created via Vec3(1.0, 2.0, 3.0)
    Borrowed {
        ptr: *mut Vec3,                // Points to component field
        validity: ValidityFlagWithMode, // Tracks read/write permissions
    },
}

#[pyclass(name = "Vec3")]
pub struct PyVec3 {
    storage: Vec3Storage,
}

impl PyVec3 {
    // Create borrowed Vec3 pointing to a component's field
    pub(crate) fn from_borrowed(ptr: *mut Vec3, validity: ValidityFlagWithMode) -> Self {
        PyVec3 {
            storage: Vec3Storage::Borrowed { ptr, validity },
        }
    }

    // Get mutable reference with safety checks
    fn as_mut(&mut self) -> PyResult<&mut Vec3> {
        match &mut self.storage {
            Vec3Storage::Owned(vec) => Ok(vec),
            Vec3Storage::Borrowed { ptr, validity } => {
                validity.check_write()?;  // Ensures mutable access allowed
                Ok(unsafe { &mut **ptr })
            }
        }
    }
}

#[pymethods]
impl PyVec3 {
    #[setter]
    pub fn set_x(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.x = value;  // Writes directly to ECS memory!
        Ok(())
    }
}
```

**Transform property returns borrowed Vec3**:
```rust
// In Transform::translation() - src/transform/transform.rs
#[getter]
pub fn translation(&self) -> PyResult<PyVec3> {
    self.check_valid()?;
    match &self.storage {
        TransformStorage::Owned(boxed) => {
            Ok(PyVec3::from(boxed.translation))  // Copy for owned
        }
        TransformStorage::Borrowed { ptr, validity } => {
            // Return borrowed Vec3 pointing directly to the field
            let translation_ptr = unsafe {
                let transform_ptr = *ptr as *const Transform;
                &(*transform_ptr).translation as *const Vec3 as *mut Vec3
            };
            Ok(PyVec3::from_borrowed(translation_ptr, validity.clone()))
        }
    }
}
```

### How It Works

1. `Query[Mut[Transform]]` iteration creates borrowed Transform pointing to ECS memory
2. `transform.translation` getter returns borrowed Vec3 pointing to transform's field
3. `translation.x = 100.0` calls `set_x()`, which writes directly to ECS memory
4. `ValidityFlagWithMode` ensures write access is allowed (would fail for read-only query)

**Result**: Field mutations persist to ECS!

```python
# This WORKS with borrowed field access:
for transform in Query[Mut[Transform]]:
    transform.translation.x = 100.0  # Writes to ECS memory
    transform.rotation.w = 0.5       # Persists!
```

### Implemented Types

Types with borrowed field access:

- **Vec2** (`src/math/vec2.rs`) - 2D vectors with `.x`, `.y` setters
- **Vec3** (`src/math/vec3.rs`) - 3D vectors with `.x`, `.y`, `.z` setters
- **Quat** (`src/math/quat.rs`) - Quaternions with `.x`, `.y`, `.z`, `.w` setters
- **Transform** (`src/transform/transform.rs`) - Uses borrowed Vec3/Quat for properties
- **PointLight** (`src/light/point_light.rs`) - Direct field mutations

### Safety Guarantees

1. **ValidityFlag**: Prevents access after system completion
2. **ValidityFlagWithMode**: Enforces read-only vs mutable access:
   - `Query[Transform]` → Borrowed fields are read-only
   - `Query[Mut[Transform]]` → Borrowed fields allow writes
3. **Pointer safety**: Only dereferenced after validity check passes
4. **Lifetime coupling**: Borrowed field lifetime tied to parent component

---

## 6. Thread Safety

PyBevy types that wrap raw pointers implement `Send + Sync` to allow them to cross thread boundaries:

```rust
// SAFETY: ValidityFlag ensures borrowed pointers only accessed during system execution
unsafe impl Send for PyVec3 {}
unsafe impl Sync for PyVec3 {}
```

**Why this is safe**:
- Pointers are never dereferenced without validity check
- Validity check fails if accessed from wrong context
- ECS scheduler ensures exclusive access during system execution

---

## Safety Precautions for Users

### Do This

**Store components temporarily during system execution**:
```python
def system(query: Query[Mut[Transform]]) -> None:
    transforms = list(query)  # OK within system
    for t in transforms:
        t.translation.x += 1.0  # OK - still in system
```

**Use multiple read-only queries**:
```python
def system(
    transforms: Query[Transform],
    lights: Query[PointLight]
) -> None:
    pass  # OK - multiple readers allowed
```

### Don't Do This

**Store system parameters globally**:
```python
stored_query = None

def system(query: Query[Transform]) -> None:
    global stored_query
    stored_query = query  # DON'T!

# Later...
stored_query[0]  # RuntimeError: Component no longer valid
```

**Multiple mutable queries for same component**:
```python
def system(
    q1: Query[Mut[Transform]],
    q2: Query[Mut[Transform]]  # RuntimeError at registration!
) -> None:
    pass
```

**Mix mutable and immutable queries**:
```python
def system(
    read: Query[Transform],
    write: Query[Mut[Transform]]  # RuntimeError at registration!
) -> None:
    pass
```

---

## Error Messages

PyBevy provides clear error messages for safety violations:

**Use-after-free attempt**:
```
RuntimeError: Component access is no longer valid (system has completed execution)
```

**Multiple mutable queries**:
```
RuntimeError: System 'update_positions' has conflicting component access:
 - Parameter 1 requests mutable access to Transform
 - Parameter 0 already has mutable access to Transform
 Rust's borrowing rules forbid multiple mutable references or
 mixing mutable and immutable references to the same data.
```

**Write to read-only field**:
```
RuntimeError: Cannot write to component - query does not have mutable access
```

---

## Performance Impact

| Safety Mechanism | Overhead | Impact |
|------------------|----------|--------|
| Validity check | ~4ns per access | ~4-8% of query iteration |
| Parameter validation | One-time at registration | Zero runtime cost |
| Borrowed field access | Zero (inline match) | No overhead vs direct access |

**Comparison**: Python's method call overhead is ~100-500ns, making the 4ns validity check negligible (~1% of method call cost).

---

## Implementation Details

### Key Files

- **Core Storage Layer** (`crates/pybevy_core/src/`):
  - `validity_guard.rs` - ValidityFlag, ValidityGuard (RAII), AccessMode
  - `pycomponent.rs` - Owned/Borrowed component storage
  - `value_storage.rs` - Copy-type field storage (Vec3, Quat, f32, etc.)
  - `field_storage.rs` - Non-Copy field storage (TextureAtlas, WindowResolution, etc.)
  - `list_storage.rs` - Vec<T> field storage with Python list interface
  - `storage/pyasset.rs` - Asset storage (read-only vs mutable variants)
- **Parameter Validation**: `src/ecs/dynamic_system.rs` (`validate_parameters()`, `validate_component_access_internal()`)
- **Borrowed Fields**: `src/math/vec2.rs`, `src/math/vec3.rs`, `src/math/quat.rs`
- **Component Pattern**: `src/transform/transform.rs`, `src/light/point_light.rs`
- **Python Safety Tests**: `tests/safety/` (15 test files)
- **Rust Unit Tests**: `cargo test -p pybevy_core` (63 tests covering all storage types)

### Related Documentation

- **[Enum Component Pattern](tech/enum-component-pattern.md)** - Detailed guide to borrowed field pattern

---

## 7. Rust Unit Tests (Storage Layer)

**Purpose**: Validate the core unsafe storage layer in pure Rust, independently from Python

### Overview

The `pybevy_core` crate contains all the unsafe pointer logic (owned/borrowed storage, validity flags, field borrowing). These are tested with pure Rust unit tests that can run without the Python interpreter.

```bash
cargo test -p pybevy_core    # 63 tests, ~0s runtime
```

### Coverage

| Module | Tests | What's Tested |
|--------|-------|---------------|
| `validity_guard` | 10 | Flag lifecycle, read/write/invalid modes, RAII guard drop, clone propagation, ValidityFlagWithMode |
| `pycomponent` | 9 | Owned/borrowed modes, mutation persistence, validity enforcement, write permissions, into_owned |
| `value_storage` | 8 | Same as pycomponent for Copy types (Vec3, f32, etc.) |
| `field_storage` | 12 | Owned/borrowed, borrow_field chains (owned→field, borrowed→field), mutation persistence through nested borrows, Drop invalidation, clone independence |
| `list_storage` | 11 | Vec storage, mutation persistence, validity/write enforcement, normalize_index edge cases |
| `pyasset` | 8 | Read-only vs mutable variants, take/consume, validity enforcement |
| `handle` | 2 | UUID handling |
| `global_registry` | 2 | Registry basics |
| **Total** | **63** | |

### Key Safety Properties Tested

- **Use-after-free prevention**: Validity flag becomes Invalid after guard drops; access raises error
- **Write permission enforcement**: Read-only borrows reject mutation attempts
- **Mutation persistence**: Borrowed storage mutations propagate through raw pointers to original data
- **Drop invalidation**: Owned storage invalidates its validity flag before freeing memory
- **Nested borrow chains**: Field borrows from borrowed parents share the same validity flag
- **Clone independence**: Cloned owned storage has independent data and validity flags

---

## 8. AddressSanitizer Testing

**Purpose**: Detect memory errors (use-after-free, buffer overflow, etc.) at runtime using compiler instrumentation

### Running

```bash
./scripts/check_asan.sh                          # Phase 1 only (Rust unit tests under ASan)
./scripts/check_asan.sh --full                   # Phase 1 + Phase 2 (build .so + run pytest)
./scripts/check_asan.sh --full tests/safety/     # Phase 2 with specific test directory
./scripts/check_asan.sh --full -k test_transform # Phase 2 with pytest args
```

### Requirements

```bash
rustup toolchain install nightly
sudo apt install libasan8    # only needed for --full (Phase 2)
```

### What It Does

1. **Phase 1** (default): Runs `cargo +nightly test -p pybevy_core` with `-Z sanitizer=address` — validates the Rust storage layer under ASan. Always works because `pybevy_core` has minimal dependencies.
2. **Phase 2** (`--full`): Builds the full PyO3 `.so` with ASan instrumentation via `maturin develop`, then runs `pytest` with `LD_PRELOAD=libasan.so` — catches memory errors during actual Python-to-Rust interaction.
3. **Cleanup** (Phase 2 only): Rebuilds without ASan to restore the normal dev environment.

### What ASan Detects

- Use-after-free (dangling pointer dereference)
- Heap/stack buffer overflow
- Stack use after return
- Memory leaks (disabled by default — Python's GC makes this noisy)

### Limitations

- Requires Rust nightly toolchain
- Phase 2 may fail on some nightly versions due to transitive dependency breakage (e.g., `zerocopy`, `futures-util` not compiling on bleeding-edge nightly). Phase 1 results are still valid in this case.
- Miri (`cargo miri`) cannot be used because `pybevy_core` depends on `pyo3`/`bevy` which have FFI that Miri can't interpret
- ASan adds ~2x runtime overhead (not for production use)

---

## Summary

PyBevy achieves production-ready safety through:

1. **Runtime validity checking** - Prevents use-after-free (~4ns overhead)
2. **Comprehensive parameter validation** - Enforces Rust borrowing rules for all parameter types (zero runtime cost)
   - Query/View component access
   - Res/ResMut resource access
   - Assets[T] asset access
   - World exclusive access
3. **Standard Python reference semantics** - Within parameters, follows normal Python behavior
4. **Free-threading compatible** - Works with Python 3.13+ without GIL
5. **Async system detection** - Blocks async functions at registration (would break ValidityFlag)
6. **Borrowed field access** - Direct mutations persist safely (zero overhead)
7. **Rust unit tests** - 63 tests covering all storage types in `pybevy_core`
8. **AddressSanitizer support** - `scripts/check_asan.sh` for runtime memory error detection

### Test Coverage

| Layer | What's Tested | Count |
|-------|---------------|-------|
| Rust storage (pybevy_core) | Validity flags, owned/borrowed storage, field borrows, Drop safety | 63 tests |
| Python safety (tests/safety/) | Parameter conflicts, aliasing, async detection, field mutations | 15 test files |
| ASan (scripts/check_asan.sh) | Runtime memory errors across Rust + Python boundary | On-demand |

These mechanisms ensure that PyBevy maintains Rust's safety guarantees at runtime, making it safe for production use while keeping performance impact minimal.
