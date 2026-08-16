# Queries Guide

For Python query syntax (`Query[T]`, `Mut[T]`, filters, `Optional`, borrow rules), see `guide://patterns` (Queries section).

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
def batch_update(view: View[Mut[Transform], With[Marker]]) -> None:
    pos = view.column_mut(Transform)
    pos.translation.y = expr.sin(time_val) * 10.0  # Updates ALL matching entities at once
```

Entity IDs are available via `batch.entities()` when using `iter_batches()`:

```python
def batch_with_entities(view: View[Entity, Mut[Transform]], commands: Commands) -> None:
    for batch in view.iter_batches():
        entities = batch.entities()  # list[Entity] in same order as column data
        col = batch.column_mut(Transform)
        # entities[i] corresponds to column data at index i
```

View commonly ranges from about 5x faster for conditional work to 20–25x for
pure column math. Measure the actual workload; see `guide://performance`.

## Common MCP Workflow

1. `query_entities {"with": ["Transform"]}` - Find entities
2. `get_component_schema {"name": "Transform"}` - See available fields
3. `set_component {"entity": N, "component": "Transform", "fields": {...}}` - Modify
4. `capture_screenshot` - Verify visually

## Gotcha: Resource Entities

Resources live on entities (bevy 0.19), so a bare `Query[Entity]` also matches them. Use `Without[IsResource]` when an operation applies to every ordinary entity, or scope the query with a domain component:

```python
from pybevy.ecs import IsResource

def cleanup_all(commands: Commands, query: Query[Entity, Without[IsResource]]) -> None:
    for entity in query:
        commands.entity(entity).despawn()

def cleanup_enemies(commands: Commands, query: Query[Entity, With[Enemy]]) -> None:
    for e in query:
        commands.entity(e).despawn()
```

For exclusive-world inspection, `world.resource_entity(Time)` returns Time's
stable entity and `world.resource_entities()` snapshots every resource
component ID/entity pair. `IsResource` is engine-managed and cannot be
constructed or inserted from Python.

Native resource wrappers can also be queried on that entity:

```python
def inspect_time(query: Query[tuple[Entity, Time]]) -> None:
    for resource_entity, time in query:
        print(resource_entity, time.elapsed_secs())
```

This mirrors Bevy 0.19's `Query<(Entity, &Time)>` behavior for resource
entities.

Prefer `Res[T]` and `ResMut[T]` for ordinary singleton access. They share the
same scheduler access as resource queries, so `Res[T]` correctly conflicts
with `Query[Mut[T]]` unless the query excludes resource entities with
`Without[IsResource]`.
