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

View is 30-50x faster than Query for bulk operations. See `guide://performance`.

## Common MCP Workflow

1. `query_entities {"with": ["Transform"]}` - Find entities
2. `get_component_schema {"name": "Transform"}` - See available fields
3. `set_component {"entity_id": N, "component": "Transform", "fields": {...}}` - Modify
4. `capture_screenshot` - Verify visually

## Gotcha: Resource Entities

Resources live on entities (bevy 0.19), so a bare `Query[Entity]` also matches them. Despawning one destroys the resource and panics later. Always scope entity queries with a component:

```python
def cleanup(commands: Commands, query: Query[Entity, With[Enemy]]) -> None:  # not Query[Entity]
    for e in query:
        commands.entity(e).despawn()
```
