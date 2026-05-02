# Patterns Guide (Extended)

ECS essentials (entities, components, systems, queries, borrow rules, resources, messages, hierarchies) and coding patterns (entry points, with_children, entity lifecycle, state machines, material swapping).

## App Boilerplate

Every PyBevy scene needs this structure. The `@entrypoint` function receives an `App`, registers plugins and systems, and returns it.

```python
import math
from dataclasses import dataclass
from pybevy.prelude import *

# -- Components --
@component
class MyMarker(Component):
    pass

@component
@dataclass
class Velocity(Component):
    speed: float = 1.0

# -- Startup system --
def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Camera — default to eye-level, NOT overhead
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(8, 3, 8).looking_at(Vec3(0, 1.5, 0), Vec3.Y),
        Bloom(intensity=0.15),
    )
    # Distance fog (mandatory for 3D scenes)
    commands.spawn(DistanceFog(
        color=Color.srgb(0.7, 0.8, 0.95),
        falloff=FogFalloff.Exponential(0.005),  # or FogFalloff.Linear(start, end)
    ))
    # WARNING: Ambient 300+ for outdoor, 500+ for indoor/cave.
    # Material base_color >= 0.20 for large surfaces. Start bright, dim later.
    # See guide://scene-quality for minimum lighting floors.
    # Geometry, lights, etc.
    ...

# -- Update system --
def animate(
    query: Query[Mut[Transform], With[MyMarker]],
    time: Res[Time],
) -> None:
    for transform in query:
        transform.translation.y = math.sin(time.elapsed_secs())

# -- Entrypoint (MUST accept App and return App) --
@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, animate)
    )

if __name__ == "__main__":
    main().run()
```

**Key rules:**
- `@entrypoint` signature is `def main(app: App) -> App:` — not `def main():`
- **MUST** end with `if __name__ == "__main__": main().run()` — without this the scene won't launch
- Register systems with `app.add_systems(Stage, fn)` — there is no `@app.system()` decorator
- Multiple systems in one stage: `app.add_systems(Update, sys1, sys2, sys3)`
- Extra plugins go in the entrypoint before `add_systems`
- Custom plugins require the `@plugin` decorator AND `Plugin` inheritance:
  ```python
  @plugin
  class MyPlugin(Plugin):
      def build(self, app: App) -> None:
          app.add_systems(Update, my_system)
  ```
- Custom `Plugin.build()` works with hot reload — systems and resources registered inside build() are re-captured on reload

## ECS Essentials

PyBevy uses Bevy's Entity Component System (ECS). Entities are IDs, components are data attached to entities, and systems are functions that process entities based on their components.

### Entities

Entities are spawned via `Commands` and identified by an `Entity` handle.

```python
from pybevy.prelude import *

def setup(commands: Commands) -> None:
    # Spawn an entity with components
    commands.spawn(Transform.from_xyz(0, 1, 0), PointLight(intensity=1000.0))

    # Spawn and get the Entity ID
    entity = commands.spawn(Transform(), Name("Player")).id()

    # Spawn an empty entity, then insert components later
    e = commands.spawn_empty().id()
    commands.entity(e).insert(Transform(), Name("Deferred"))

    # Despawn an entity
    commands.despawn(entity)
```

Use `Name` to give entities stable human-readable identifiers. Names persist across hot reloads and are queryable via MCP tools.

```python
commands.spawn(Camera3d(), Transform.from_xyz(0, 5, -10), Name("MainCamera"))
```

### Components

#### Built-in Components

PyBevy wraps Bevy components as Python classes:

- **Transform** -- position (`translation`), `rotation`, `scale`
- **PointLight**, **DirectionalLight**, **SpotLight** -- light sources
- **Camera3d**, **Camera2d** -- cameras
- **Mesh3d**, **MeshMaterial3d** -- mesh and material handles
- **Name** -- human-readable entity name
- **Visibility** -- `INHERITED`, `VISIBLE`, or `HIDDEN`

#### Custom Components

Define custom components with the `@component` decorator and `Component` base class.

```python
from dataclasses import dataclass

# Component with fields -- use @dataclass
@component
@dataclass
class Player(Component):
    player_id: int
    health: float

# Marker component (no fields) -- no @dataclass needed
@component
class Enemy(Component):
    pass
```

Field access works directly in queries:

```python
def damage_players(query: Query[Mut[Player]]) -> None:
    for player in query:
        player.health -= 10.0
```

**Storage modes:**
- **Default (wrapper)**: `int`, `float`, `bool`, `Vec3`, `Vec2` — fast, supports View batch execution and Numba JIT
- **Python storage**: `str`, `list`, `dict`, custom classes — use `@component(storage="python")`. Slower (no View/Numba), but supports any Python type.

```python
@component(storage="python")
@dataclass
class Inventory(Component):
    name: str = ""
    items: list[str] = field(default_factory=list)
```

### Entity Hierarchies

Parent-child relationships use `ChildOf`. Children inherit their parent's `Transform`.

#### Explicit ChildOf

```python
def setup(commands: Commands) -> None:
    parent = commands.spawn(Transform(), Name("Root")).id()
    commands.spawn(Transform.from_xyz(0, 1, 0), ChildOf(parent))
```

#### Builder Pattern with with_children

```python
def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    mesh = meshes.add(Cuboid(0.5, 0.5, 0.5))
    mat = materials.add(StandardMaterial(base_color=Color.srgb(0.8, 0.2, 0.2)))

    # Single child
    commands.spawn(Transform()).with_children(lambda cb:
        cb.spawn(Mesh3d(mesh), MeshMaterial3d(mat), Transform.from_xyz(0, 1, 0))
    )

    # Multiple children -- wrap in a tuple
    commands.spawn(Transform(), Name("Robot")).with_children(lambda cb: (
        cb.spawn(Mesh3d(mesh), Transform.from_xyz(0, 2, 0)),
        cb.spawn(Mesh3d(mesh), Transform.from_xyz(1, 0, 0)),
        cb.spawn(Mesh3d(mesh), Transform.from_xyz(-1, 0, 0)),
    ))
```

**Closure capture in loops:** When calling `with_children` inside a `for` loop, you **must** capture loop variables via lambda default arguments. Otherwise Python's late binding means every child lambda uses the *last* iteration's values:

```python
# WRONG: all arms get the last config's radius
for cfg in arm_configs:
    commands.spawn(Transform(), RotatingArm(speed=cfg["speed"])).with_children(
        lambda parent: (
            parent.spawn(Transform.from_xyz(cfg["radius"], 0, 0)),  # always last value!
        )
    )

# FIX: capture via default argument
for cfg in arm_configs:
    commands.spawn(Transform(), RotatingArm(speed=cfg["speed"])).with_children(
        lambda parent, r=cfg["radius"], mat=arm_mat: (
            parent.spawn(Transform.from_xyz(r, 0, 0), Mesh3d(mesh), MeshMaterial3d(mat)),
        )
    )
```

Capture **every** variable from the loop scope that the lambda references — including material handles, mesh handles, and config values.

#### Modifying Hierarchy at Runtime

```python
def reparent(commands: Commands, query: Query[Entity, With[Enemy]]) -> None:
    new_parent = commands.spawn(Transform()).id()
    for entity in query:
        commands.entity(entity).set_parent(new_parent)

def orphan(commands: Commands, parent_query: Query[tuple[Entity, Children]]) -> None:
    for entity, children in parent_query:
        for child in children:
            commands.entity(child).remove_parent()
```

#### Querying Children

The `Children` component is auto-managed. Query it to traverse hierarchies:

```python
def print_tree(query: Query[tuple[Name, Children]]) -> None:
    for name, children in query:
        for child_entity in children:
            print(f"{name.as_str()} has child {child_entity}")
```

### Systems

Systems are plain functions. Parameter type hints control what data is injected.

#### Schedule Stages

```python
@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)          # Runs once at start
        .add_systems(Update, game_logic)      # Runs every frame
        .add_systems(Last, cleanup)           # Runs after Update each frame
    )
```

| Stage | When | Use For |
|-------|------|---------|
| `Startup` | Once at launch | Scene setup, resource init |
| `First` | Every frame, first | Early frame logic |
| `Update` | Every frame | Main game logic |
| `Last` | Every frame, last | Cleanup, state sync |
| `FixedUpdate` | Fixed timestep | Physics, deterministic logic |

#### System Parameters

```python
def my_system(
    commands: Commands,                              # Spawn/despawn entities
    query: Query[Mut[Transform], With[Player]],      # Read/write components
    time: Res[Time],                                 # Read-only resource
    state: ResMut[GameState],                        # Mutable resource
    writer: MessageWriter[DamageEvent],              # Send messages
    reader: MessageReader[DamageEvent],              # Read messages
) -> None:
    ...
```

#### Conditional Systems

```python
from pybevy.ecs import run_if

def is_game_active(state: Res[GameState]) -> bool:
    return state.active

app.add_systems(Update, run_if(game_logic, is_game_active))

# Combinators
app.add_systems(Update, run_if(game_logic, cond_a).and_(cond_b))
app.add_systems(Update, run_if(game_logic, cond_a).or_(cond_b))
app.add_systems(Update, run_if(game_logic, cond_a).not_())
```

### Queries

#### Single Component

```python
# Read-only
def read_positions(query: Query[Transform]) -> None:
    for transform in query:
        print(transform.translation)

# Mutable -- Mut[] required for modifications
def move_up(query: Query[Mut[Transform]]) -> None:
    for transform in query:
        transform.translation.y += 1.0
```

#### Multiple Components

Use `tuple[]` for multiple components:

```python
def apply_velocity(query: Query[tuple[Mut[Transform], Velocity]], time: Res[Time]) -> None:
    for transform, vel in query:
        transform.translation.x += vel.x * time.delta_secs()
        transform.translation.y += vel.y * time.delta_secs()
```

#### Filters

```python
# With -- only entities that have the component
Query[Transform, With[Player]]

# Without -- exclude entities that have the component
Query[Transform, Without[Enemy]]

# Multiple filters -- use tuple[]
Query[Transform, tuple[With[Player], Without[Dead]]]

# Multiple components in With
Query[Transform, With[tuple[Player, Visible]]]

# Changed -- only if component was modified since last run
Query[Mut[Transform], Changed[Transform]]

# Added -- only if component was added since last run
Query[Player, Added[Player]]

# Optional -- returns component or None
Query[tuple[Transform, Optional[Visibility]]]  # Visibility | None per entity
Query[Optional[Mut[Transform]]]                # mutable when present
```

#### Borrow Rules (IMPORTANT)

A system cannot have `Mut[T]` access to a component in one query and **any** access (mutable or read-only) to the same component in another query, unless disjointness is **proven by `Without` filters**. Different `With` filters alone (e.g., `With[A]` vs `With[B]`) are **not enough** — Bevy cannot prove those sets don't overlap.

```python
# WRONG — panics even though A and B are logically disjoint:
def bad(q1: Query[Mut[Transform], With[A]], q2: Query[Mut[Transform], With[B]]) -> None: ...

# ALSO WRONG — Mut + read-only is still a conflict:
def bad2(q1: Query[Mut[Transform], With[A]], q2: Query[Transform, With[B]]) -> None: ...

# OK — Without proves disjointness:
def ok(q1: Query[Mut[Transform], tuple[With[A], Without[B]]], q2: Query[Mut[Transform], tuple[With[B], Without[A]]]) -> None: ...

# BEST — split into separate systems (simplest, always works):
def move_a(q: Query[Mut[Transform], With[A]]) -> None: ...
def read_b(q: Query[Transform, With[B]]) -> None: ...
```

Querying a component (e.g., `Query[tuple[Transform, TagA]]`) automatically implies `With[TagA]` — so `Without[TagA]` on another query proves disjointness. When in doubt, split into separate systems or use the Resource Flag Pattern (see below).

**Change detection note:** Custom Python components using PyObject storage do NOT trigger Bevy's `Changed[T]` filter when fields are mutated. `Added[T]` still works for newly spawned components.
#### Query Methods

```python
def find_player(query: Query[Transform, With[Player]]) -> None:
    # Get exactly one result (errors if 0 or 2+)
    transform = query.single()

    # Check if empty
    if query.is_empty():
        return

    # Get by specific entity
    result = query.get(some_entity)
    if result is not None:
        print(result.translation)
```

### Resources

Resources are global singletons accessible from any system.

#### Built-in Resources

```python
def use_time(time: Res[Time]) -> None:
    elapsed = time.elapsed_secs()
    dt = time.delta_secs()

def load_model(asset_server: Res[AssetServer]) -> None:
    handle = asset_server.load("models/character.gltf#Scene0")

    # For images and audio, use convenience methods (no asset_type needed):
    texture = asset_server.load_image("textures/my_texture.png")
    sound = asset_server.load_audio("sounds/click.ogg")
```

#### Custom Resources

```python
class GameState(Resource):
    score: int = 0
    level: int = 1
    active: bool = True

# Register with the app
app.insert_resource(GameState())

# Read-only access
def display_score(state: Res[GameState]) -> None:
    print(f"Score: {state.score}")

# Mutable access
def update_score(state: ResMut[GameState], query: Query[Player]) -> None:
    state.score = sum(int(p.health) for p in query)
```

Resources can also be inserted from systems via `Commands`:

```python
def setup(commands: Commands) -> None:
    commands.insert_resource(GameState())
```

### Messages

Messages provide inter-system communication using double-buffering: messages written in frame N are readable in frame N+1.

#### Defining and Registering

```python
from dataclasses import dataclass

@dataclass
class DamageEvent(Message):
    entity_id: int
    amount: float

# Register with the app before use
app.add_message(DamageEvent)
```

#### Sending and Receiving

```python
def send_damage(writer: MessageWriter[DamageEvent]) -> None:
    writer.write(DamageEvent(entity_id=42, amount=25.0))

def receive_damage(reader: MessageReader[DamageEvent], query: Query[Mut[Player]]) -> None:
    for event in reader:
        for player in query:
            if player.player_id == event.entity_id:
                player.health -= event.amount
```

`MessageWriter` methods: `write(msg)`, `write_batch([msg1, msg2])`, `write_default()`.
`MessageReader` methods: iteration via `for msg in reader`, `is_empty()`, `len()`.

### Observers

Observers react to events or lifecycle hooks without polling. Register with `app.add_observer()`.

```python
# React to a custom event
def on_player_died(trigger: On[PlayerDied]) -> None:
    event = trigger.event()
    entity = trigger.entity()

app.add_observer(on_player_died)

# Lifecycle hooks: Add, Insert, Remove, Replace, Despawn
def on_transform_added(trigger: On[Add, Transform]) -> None:
    entity = trigger.entity()

app.add_observer(on_transform_added)
```

The `On` parameter supports filtering: `On[EventType, ComponentType]` only triggers for entities with that component. Use `On[EventType, tuple[A, B]]` to require multiple components.

## Patterns

### Mesh Primitives

See `guide://mesh` for the full primitives table, builder patterns, and custom mesh creation. Key shapes: `Cuboid(x,y,z)`, `Sphere(radius)`, `Cylinder(radius, height)`, `Cone(radius, height)`, `Plane3d(normal, half_size=Vec2(w,h))`, `Torus(inner, outer)`.

**Origin matters for positioning:** `Cone` has its base at origin (tip points +Y). `Cylinder` is centered (extends +/-half-height).

### `import math` and `math.tau`

The scene template's imports should include `import math` — nearly every animated scene needs `math.sin`, `math.cos`, or `Quat.from_euler` (which takes radians).

For periodic motion (spinning, orbiting, pulsing), `math.tau` (= 2pi) is cleaner than `2 * math.pi`:

```python
import math

# Rotation speed from period: one full rotation every 8 seconds
speed = math.tau / 8.0

# In Update system:
transform.rotation = Quat.from_euler(EulerRot.XYZ, 0.0, speed * time.elapsed_secs(), 0.0)
```

### Query Iteration with `enumerate()`

Queries are standard Python iterables. Use `enumerate()` when you need a per-entity index (e.g., for phase offsets in animations):

```python
def sway_lanterns(
    query: Query[Mut[Transform], With[Lantern]],
    time: Res[Time],
) -> None:
    t = time.elapsed_secs()
    for i, transform in enumerate(query):
        phase = float(i) * 1.7
        transform.translation.y += math.sin(t + phase) * 0.01
```

**Note:** `Entity` is an opaque handle — it has no `.index()` or `.id()` method. Use `enumerate()` instead when you need a numeric index.

### Runtime Entity Lifecycle (Spawn/Despawn in Update)

Many scenes need temporary entities spawned and despawned at runtime — projectiles, VFX, timed effects, pooled objects.

#### Basic Pattern: Spawn + Marker + Despawn

```python
@component
class Projectile(Component):
    pass

@component
@dataclass
class Lifetime(Component):
    remaining: float = 2.0

# Spawn in response to game logic
def fire_projectile(
    commands: Commands,
    assets: Res[ProjectileAssets],  # Pre-cached handles (see performance guide)
) -> None:
    if should_fire:
        commands.spawn(
            Mesh3d(assets.bullet_mesh), MeshMaterial3d(assets.bullet_mat),
            Transform.from_xyz(0, 1, 0),
            Projectile(),
            Lifetime(remaining=2.0),
            Name("bullet"),
        )

# Despawn when lifetime expires
def despawn_expired(
    commands: Commands,
    query: Query[tuple[Entity, Mut[Lifetime]]],
    time: Res[Time],
) -> None:
    dt = time.delta_secs()
    for entity, lifetime in query:
        lifetime.remaining -= dt
        if lifetime.remaining <= 0.0:
            commands.entity(entity).despawn()
```

#### State Machine Pattern (multi-phase effects)

For app-wide game flow (Menu → Playing → Paused → GameOver), use PyBevy's built-in state machine system with `@state`, `OnEnter`/`OnExit` schedules, and `run_if(in_state(...))`. See `guide://state-machines` for the full API.

For effects with distinct phases (telegraph -> strike -> cooldown), use a resource to track state:

```python
@resource
@dataclass
class EffectState(Resource):
    phase: str = "idle"      # idle -> telegraph -> active -> cooldown
    timer: float = 0.0
    spawned: bool = False

def effect_controller(
    commands: Commands,
    state: ResMut[EffectState],
    assets: Res[EffectAssets],
    time: Res[Time],
) -> None:
    state.timer += time.delta_secs()

    if state.phase == "idle":
        if state.timer >= 1.5:
            state.phase = "telegraph"
            state.timer = 0.0
            state.spawned = False

    elif state.phase == "telegraph":
        if not state.spawned:
            commands.spawn(...)  # Warning indicator
            state.spawned = True
        if state.timer >= 1.0:
            state.phase = "active"
            state.timer = 0.0
            state.spawned = False

    # ... etc
```

Use separate marker components per phase (`WarningMarker`, `ActiveEffect`, `Flash`) so animate/despawn systems can target each independently.

#### Key Rules

1. **Cache asset handles** — Never call `meshes.add()` / `materials.add()` in Update systems. See `guide://performance`.
2. **Use marker components** — Tag spawned entities so you can query and despawn them by type.
3. **Despawn in the right phase** — Don't leave orphaned entities. Use state transitions to trigger cleanup.
4. **PointLights don't need mesh handles** — Lights are components, no caching needed. Still tag with markers for cleanup.

### Component vs Resource Field Types

**Components** and **resources** have different storage architectures and different field type rules.

#### `@component` fields — primitives and Vec3/Vec2 (by default)

Components are stored in Bevy's ECS archetype tables using fixed-size wrapper types. By default, `int`, `float`, `bool`, `Vec3`, and `Vec2` fields are allowed. Non-primitive fields require explicit opt-in:

```python
@component
@dataclass
class Velocity(Component):       # OK: primitive fields
    speed: float = 0.0
    direction: Vec3 = field(default_factory=lambda: Vec3.ZERO)

@component
@dataclass
class Particle(Component):       # OK: Vec3 field with borrowed writeback
    position: Vec3 = field(default_factory=lambda: Vec3.ZERO)
    velocity: Vec3 = field(default_factory=lambda: Vec3.ZERO)
    mass: float = 1.0

@component(storage="python")
@dataclass
class Inventory(Component):      # OK: opted into PyObject storage
    items: list[str] = field(default_factory=list)
```

Vec3/Vec2 fields support borrowed writeback — `comp.position.x = 5.0` writes directly to ECS memory. They work with Query, View API batch expressions, Numba JIT, and `from_numpy()` batch spawning (with `(N, 3)` or `(N, 2)` shaped arrays).

**Note:** Vec3/Vec2 are mutable, so use `field(default_factory=lambda: Vec3.ZERO)` for defaults.

Using `storage="python"` disables View batch execution and Numba JIT for that component. Only opt in when needed.

#### `@resource` fields — any Python type

Resources are stored as plain Python objects in a side HashMap — they never cross a serialization boundary. **Any Python type works**, including `list`, `dict`, `Queue`, `Lock`, custom classes, etc.:

```python
import queue
import threading

@resource
class GameState(Resource):
    def __init__(self):
        self.score = 0
        self.level = 1
        self.positions: list[tuple[float, float]] = []

@resource
class ThreadBridge(Resource):
    """Share channels between a background thread and ECS systems."""
    def __init__(self):
        self.queue = queue.Queue()
        self.lock = threading.Lock()
        self.shared_state = {"status": "idle"}
```

This is the idiomatic way to bridge background threads with ECS — store the shared `Queue`/`Lock` in a resource so systems access them via `Res[T]`/`ResMut[T]` instead of module globals.

**Asset handles are `int`:** `meshes.add()` and `materials.add()` return `int` handle IDs, so storing them in `@resource` fields works naturally. See `guide://performance` for the cached asset pattern.

### Resource Flag Pattern (Cross-System Communication)

When two systems need to interact but can't share mutable access to the same component (e.g., "detect hazard collision" needs `Query[Hazard, GridPos]` + reading player position, while "reset player" needs `Query[Mut[Transform], With[Player]]`), use a **resource flag** to decouple them:

```python
@resource
@dataclass
class GameState(Resource):
    reset_player: bool = False    # Flag: set by one system, consumed by another
    player_gx: int = 0
    player_gz: int = 0

# System 1: Detect (reads hazards + game state, sets flag)
def check_hazard_hit(
    hazard_q: Query[tuple[Hazard, GridPos], With[HazardGlow]],
    time: Res[Time],
    state: ResMut[GameState],
) -> None:
    for hazard, gpos in hazard_q:
        if is_active(hazard, time) and state.player_gx == gpos.x and state.player_gz == gpos.z:
            state.reset_player = True
            state.player_gx = START_X
            state.player_gz = START_Z
            return

# System 2: React (reads flag, mutates player transform)
def reset_player(
    player_q: Query[Mut[Transform], With[Player]],
    state: ResMut[GameState],
) -> None:
    if not state.reset_player:
        return
    state.reset_player = False
    for t in player_q:
        t.translation = grid_to_world(state.player_gx, state.player_gz)
```

**When to use this:** Any time system A detects a condition (collision, timer, input) and system B needs to mutate entities that A can't access due to borrow conflicts. Common cases:
- Hazard/trap -> player reset
- Projectile hit -> enemy damage
- Trigger zone -> spawn/despawn effects

**Alternative:** If both systems only need read access to the conflicting component, use `Query[Transform]` (no `Mut`) in the detection system and `Query[Mut[Transform]]` in the reaction system — no flag needed.

For disjoint query patterns (`Without` filters to resolve borrow conflicts within a single system), see the Borrow Rules section above.

### Runtime Material Swapping

`MeshMaterial3d` and `Mesh3d` are **immutable newtype wrappers** around `Handle`. You cannot set fields on them — `.material`, `.handle`, `.0` all fail:

```python
# WRONG: All of these fail at runtime
mat_comp.material = new_handle   # AttributeError: no attribute 'material'
mat_comp.0 = new_handle          # SyntaxError: invalid syntax
mat_comp.handle = new_handle     # AttributeError
```

**Workaround:** Use `commands.entity().insert()` to replace the entire component:

```python
def swap_materials(
    commands: Commands,
    query: Query[tuple[Entity, MyMarker]],
    assets: Res[CachedAssets],
    time: Res[Time],
) -> None:
    phase = int(time.elapsed_secs() / 3.0) % 2
    for entity, marker in query:
        handle = assets.mat_a if phase == 0 else assets.mat_b
        commands.entity(entity).insert(MeshMaterial3d(handle))
```

**Important:** This requires `Entity` in the query tuple (not `Mut[MeshMaterial3d]`). Cache material handles in a `@resource` — never call `materials.add()` in Update.

For custom shader materials, see `guide://shaders` for the `@material` decorator.

### Runtime Component Add/Remove

Add or remove components on existing entities at runtime:

```python
def pickup(commands: Commands, query: Query[tuple[Entity, Item], With[NearPlayer]]) -> None:
    for entity, item in query:
        commands.entity(entity).insert(CarriedBy(player_id=1))  # add: instances
        commands.entity(entity).remove(NearPlayer)               # remove: types
```

`insert()` takes component **instances**, `remove()` takes component **types** (no parentheses).

### `from dataclasses import dataclass`

`dataclass` is **not** in `pybevy.prelude`. You must import it explicitly:

```python
from dataclasses import dataclass
from pybevy.prelude import *
```

Every `@component` with data fields needs both `@dataclass` and `@component`. Marker components (no fields) only need `@component`:

```python
@component
@dataclass
class Health(Component):       # Has fields -> needs @dataclass
    current: float = 100.0

@component
class Player(Component):       # No fields -> marker, no @dataclass needed
    pass
```

Resources can use `@dataclass` for simple primitive state, or a plain `__init__` for complex types:

```python
@resource
@dataclass
class Score(Resource):         # Simple state -> @dataclass is fine
    value: int = 0

@resource
class AssetCache(Resource):    # Complex types -> use __init__
    def __init__(self):
        self.meshes: dict[str, int] = {}
```

### Parent Rotation for Grouped Animation

To rotate (or animate) a group of objects together, spawn a parent entity with a marker component and a `Transform`, attach children via `with_children`, then rotate the parent in an Update system. All children inherit the parent's transform:

```python
@component
class StarMap(Component):
    pass

# Startup: spawn parent with children
commands.spawn(
    Transform.from_xyz(0.0, 8.0, 0.0),
    StarMap(),
    Name("star_map"),
).with_children(
    lambda parent: (
        parent.spawn(Mesh3d(ring_mesh), MeshMaterial3d(brass_mat), Transform.IDENTITY),
        parent.spawn(Mesh3d(orb_mesh), MeshMaterial3d(glow_mat), Transform.IDENTITY),
        parent.spawn(Mesh3d(star_mesh), MeshMaterial3d(star_mat), Transform.from_xyz(2.0, 0.5, 0.0)),
    )
)

# Update: rotate the parent — all children follow
def rotate_star_map(
    query: Query[Mut[Transform], With[StarMap]],
    time: Res[Time],
) -> None:
    speed = math.tau / 60.0  # one full rotation per 60 seconds
    t = time.elapsed_secs()
    for transform in query:
        transform.rotation = Quat.from_euler(EulerRot.XYZ, 0.0, speed * t, 0.0)
```

**Use cases:** rotating star maps, spinning chandeliers, orbiting ring assemblies, turrets with multiple parts, any multi-mesh object that moves as a unit.

**Tip:** Children can have their own local transforms (offsets, tilts) — these are relative to the parent. Only the parent needs the animation system.

### Pivot Point Animation (Edge Rotation)

Bevy meshes rotate around their origin (geometric center). Objects that should pivot from an edge (grass base, door hinge, pendulum top) need special handling.

**Approach A — Parent-child (small counts):**
Spawn a parent entity at the pivot point, child mesh offset to center:
```python
# Door hinge: pivot at left edge
commands.spawn(
    Transform.from_xyz(door_x, 0, door_z),  # Pivot point
    DoorHinge(),
).with_children(lambda parent:
    parent.spawn(
        Mesh3d(door_mesh), MeshMaterial3d(door_mat),
        Transform.from_xyz(door_width / 2, door_height / 2, 0),  # Offset to center
    )
)
```
Simple but doubles entity count. Best for < 1k entities.

**Approach B — Translation compensation (large counts, 1k+):**
Keep single entities and compute position each frame to simulate edge-pivot rotation:
```python
# Grass blade pivoting from base, angle = sway amount
pos.translation.x = orig_x + half_height * math.sin(angle)
pos.translation.y = half_height * math.cos(angle)
# Quaternion for Z-axis rotation
pos.rotation = Quat.from_rotation_z(angle)
```
Requires storing original positions in a component since transform is modified each frame. Works well with View API for 1k+ entities.

### When `run_scene` is Required (Extended)

Most changes work with `reload` (Full mode), including:
- New `@component` or `@resource` classes (registered on first use)
- Changed fields on existing `@component`/`@resource` (re-aliased by name, fresh ComponentId if structure changed)
- New or modified observers (auto re-registered)

Use `run_scene` when:
- Behavior is unexpected after `reload` (clean-slate restart)
- Plugins were removed (can't be hot-unloaded)
- The Python subprocess itself is in a bad state

**Plan ahead:** Define asset cache resources and all component types with their fields upfront before iterating on system logic. This minimizes reload overhead.

### Material Variation vs Extra Geometry

For surface variation (snow coverage, dirt, wear, moss), **prefer using different materials on existing geometry** over adding thin overlay meshes.

Thin meshes (cylinders < 0.1 height, flattened cones, thin spheres) used as surface overlays tend to look like floating discs or rings, especially on cone/sphere surfaces where the overlay can't conform to the curvature.

```python
# WRONG: Floating disc problem: thin cylinder "snow ledge" on a cone
snow_disc = meshes.add(Cylinder(radius=0.9, height=0.05))
# Looks like a floating ring around the cone

# BETTER: use different materials per section
foliage_green = materials.add(StandardMaterial(base_color=Color.srgb(0.3, 0.42, 0.25)))
foliage_frosty = materials.add(StandardMaterial(base_color=Color.srgb(0.55, 0.62, 0.55)))
foliage_snowy = materials.add(StandardMaterial(base_color=Color.srgb(0.82, 0.84, 0.88)))
# Apply graduated materials to existing cone tiers — no extra geometry needed
```

**When extra geometry works:** Flattened spheres for snow piles on flat surfaces (ground, rock tops) look fine because they sit on a flat plane rather than conforming to a curved surface.
