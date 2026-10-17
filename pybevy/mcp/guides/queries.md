# Queries Guide

For Python query syntax (`Query[T]`, `Mut[T]`, filters, `Optional`, borrow rules), see `guide://patterns` (Queries section).

Group Query and Single data with builtin `tuple[...]`; `list[...]`, `dict[...]`
and other generic origins raise `TypeError`. An iterable passed to
`query.iter_many()` must not re-enter iteration on the same query: that raises
`RuntimeError`. Scheduled queries and their inspection methods expire when the
system ends.

When calling `world.run_system_once()` or `world.run_schedule()`, use the inner
system's injected parameters. Captured outer World/Commands handles and borrowed
children raise `RuntimeError` until the nested call returns. Observer callbacks
apply the same rule on their World. This prevents a captured World from moving
component storage while an inner Query iterator still references it.

Native components with value equality compare by their current value across
owned and borrowed wrappers. For example, a queried `Name("player")` equals a
newly constructed `Name("player")` while its query is valid. An expired
borrowed component does not compare equal to an owned value.

Nested values read from a live native component are often borrowed views, even
when they look like ordinary `Vec3`, `Quat`, or `Color` values. For example,
`transform.translation` and `transform.rotation` from `Query[Transform]`
expire with the system. The same applies to nested values from a borrowed
asset. Do not retain these views in a resource, `Local`, message, event, or
module variable for a later system or frame. Take an owned snapshot while the
borrow is valid:

<!-- pybevy-snippet: typecheck -->
```python
from copy import copy

from pybevy.ecs import Query, system
from pybevy.math import Vec3
from pybevy.transform import Transform

previous_positions: list[Vec3] = []

@system
def remember_positions(query: Query[Transform]) -> None:
    for transform in query:
        previous_positions.append(copy(transform.translation))
```

The copies remain readable after the system ends. A borrowed child remains
live for in-system reads and, through `Query[Mut[T]]`, nested writes; copying it
is only needed when retaining its value beyond that lifetime. Scalars and
computed owned results already have independent lifetimes.

## MCP Query Tools

### query_entities - Filter by component presence
```
query_entities {"with": ["Transform", "PointLight"]}
→ All entities that have both Transform AND PointLight

query_entities {"with": ["Transform"], "without": ["PointLight"]}
→ Entities with Transform but NOT PointLight

query_entities {"with": ["Name"]}
→ All named entities (useful for finding addressable objects)
```

Results default to 100 entity records. Pass `"limit": 25` (maximum 1000) to
choose the returned sample size; `total_count` and `truncated` describe the full
match set.

### scene://entities - List everything
```
resources/read scene://entities
→ All entities with their component type lists
```

### scene://entity/{id_or_name} - Single entity detail
```
resources/read scene://entity/42
resources/read scene://entity/MainCamera
→ Full component values for one entity
```

Native and custom component values are returned as structured JSON fields, so
you can inspect nested values directly without parsing Python `repr` strings.

## View API (Batch Operations)

For high-performance batch operations on many entities (10k+), use View instead of Query:

```python
@system
def batch_update(view: View[Mut[Transform], With[Marker]]) -> None:
    pos = view.column_mut(Transform)
    pos.translation.y = expr.sin(time_val) * 10.0  # Updates ALL matching entities at once
```

`view.column(T)` and `view.column_mut(T)` return a lazy expression column.
Field access yields expressions; assigning to a field compiles and runs the
expression across every matching entity in one step. For a concrete
`ViewColumn` buffer, iterate with `iter_batches()` and call
`batch.column(T)` / `batch.column_mut(T)`. It has `len`/`dtype`/`stride`, Numba
zero-copy indexing, and optional JAX copy conversion, with a batch-scoped lifetime. A
concrete `ViewColumn` refuses direct NumPy conversion (`__array__` raises); a NumPy
array is a copy built with `to_contiguous_bytes()` and `numpy.frombuffer()`.
Its eager arithmetic operators and math methods (`col * 2.0`, `col.sin()`) work
only on float columns. An integer or bool column raises `TypeError` naming the
operation and dtype; compute integer fields with a lazy View expression or a
Numba kernel. `set()`, `peek()`, `to_list()`, and indexing work for every dtype.

Entity IDs are available via `batch.entities()` when using `iter_batches()`:

```python
@system
def batch_with_entities(view: View[tuple[Entity, Mut[Transform]]], commands: Commands) -> None:
    for batch in view.iter_batches():
        entities = batch.entities()  # list[Entity] in same order as column data
        col = batch.column_mut(Transform)
        # entities[i] corresponds to column data at index i
```

View commonly ranges from about 5x faster for conditional work to 20–25x for
pure column math. Measure the actual workload; see `guide://performance`.

## Disabled Entities

Every Query and View skips entities that carry `Disabled`, as Bevy's default
query filters do. To include them, name `Disabled` in the query:
`Allow[Disabled]` matches enabled and disabled entities alike,
`Has[Disabled]` does the same and reports which ones are disabled, and
`With[Disabled]` matches only the disabled ones.

<!-- pybevy-snippet: typecheck -->
```python
from pybevy.ecs import Allow, Commands, Disabled, Entity, Has, Name, Query, With, system

@system
def wake_sleepers(
    commands: Commands,
    query: Query[tuple[Entity, Name, Has[Disabled]], With[Name]],
) -> None:
    for entity, name, disabled in query:
        if disabled and str(name).startswith("sleeper"):
            commands.entity(entity).remove(Disabled)

@system
def count_everything(query: Query[Entity, tuple[With[Name], Allow[Disabled]]]) -> None:
    print(len(query))
```

`Allow` works in View filters too. It names one component and goes in the
filter position, outside `Or[...]`.

## Common MCP Workflow

1. `query_entities {"with": ["Transform"]}` - Find entities
2. `get_component_schema {"name": "Transform"}` - See available fields
3. `set_component {"entity": N, "component": "Transform", "fields": {...}}` - Modify
4. `capture_screenshot` - Verify visually

## Gotcha: Resource Entities

Resources live on entities (bevy 0.20), so a bare `Query[Entity]` also matches them. Use `Without[IsResource]` when an operation applies to every ordinary entity, or scope the query with a domain component:

```python
from pybevy.ecs import IsResource, system

@system
def cleanup_all(commands: Commands, query: Query[Entity, Without[IsResource]]) -> None:
    for entity in query:
        commands.entity(entity).despawn()

@system
def cleanup_enemies(commands: Commands, query: Query[Entity, With[Enemy]]) -> None:
    for e in query:
        commands.entity(e).despawn()
```

For exclusive-world inspection, `world.resource_entity(Time)` returns Time's
stable entity and `world.resource_entities()` snapshots every resource
component ID/entity pair. `IsResource` is engine-managed and cannot be
constructed or inserted from Python. There, read a resource with
`world.resource(T)` and write one with `world.resource_mut(T)`.

Native resource wrappers can also be queried on that entity:

```python
@system
def inspect_time(query: Query[tuple[Entity, Time]]) -> None:
    for resource_entity, time in query:
        print(resource_entity, time.elapsed_secs())
```

This mirrors Bevy 0.20's `Query<(Entity, &Time)>` behavior for resource
entities.

Parameterized asset collections use the same resource-entity path. Query them
read-only with `Query[Assets[Mesh]]` or mutably with
`Query[Mut[Assets[Mesh]]]`; the row exists only after the matching asset
resource has been initialized. `world.component_id(Assets[Mesh])` registers its
native component identity without inserting the resource, and
`world.resource_entity(Assets[Mesh])` returns `None` until the resource exists.
These queries conflict with `ResMut[Assets[Mesh]]` and `Res[Assets[Mesh]]` just
like other resource queries. Decorated material parameters keep their logical
collection filtering even when several material classes share one native
`Assets[ShaderMaterial]` resource.

Prefer `Res[T]` and `ResMut[T]` for ordinary singleton access. They share the
same scheduler access as resource queries, so `Res[T]` correctly conflicts
with `Query[Mut[T]]` unless the query excludes resource entities with
`Without[IsResource]`.

Custom resource handles returned by queries expire when their system finishes,
just like `Res[T]` and `ResMut[T]`; later reads or writes raise `RuntimeError`.

`Single[T]` skips its system or observer before the body runs unless exactly
one entity matches. This is checked on each run or trigger, including when the
body never accesses the parameter. `Query.single()` is an explicit lookup inside a running system
and raises `RuntimeError` for zero or multiple matches.

The silent skip applies to scheduled systems. An explicit
`world.run_system_once()` call, including one inside `app.world()`, raises
`RuntimeError` when a required `Single[T]` has zero or multiple matches, so the
caller can tell that the requested body and its deferred commands did not run.

Use `Optional[Single[T]]` with `from pybevy.ecs import Optional`, matching
Bevy's `Option<Single<...>>`. This is the standard `typing.Optional` re-exported
beside the ECS query types; importing it from `typing` remains equivalent. The
system runs with `None` for zero or multiple matches; exactly one match supplies
the ordinary validity-bound Single wrapper. Tuple data, filters, and `Mut`
declare the same scheduler access as a required Single. The equivalent
`Single[T] | None` spelling is also supported.

Use `into_inner()` for typed component access and tuple unpacking, matching
Bevy's extraction API. Extraction keeps the existing access mode and system
lifetime. Component fields and methods belong to the extracted value; direct
attribute access or assignment on the holder raises `AttributeError`.

<!-- pybevy-snippet: typecheck -->
```python
from pybevy.ecs import Entity, Mut, Single, system
from pybevy.transform import Transform


@system
def move(row: Single[tuple[Entity, Mut[Transform]]]) -> None:
    _entity, transform = row.into_inner()
    transform.translation.x += 1.0


@system
def move_one(row: Single[Mut[Transform]]) -> None:
    transform = row.into_inner()
    transform.translation.x += 1.0
```
