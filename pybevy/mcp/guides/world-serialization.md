# Live World Serialization

Create a `DynamicWorld`, then serialize it to capture the current reflected ECS
state in Bevy's `.scn.ron` format:

```python
from pybevy.ecs import World
from pybevy.world_serialization import DynamicWorld

def capture(world: World) -> None:
    snapshot = DynamicWorld.from_world(world)
    ron_text = snapshot.serialize(world)
```

`serialize()` returns RON text. Keep the `DynamicWorld` if the in-memory
snapshot must be serialized again.

`DynamicWorld.from_world()` extracts all entities, components, and resources
that Bevy can access through `AppTypeRegistry` reflection. Wrapper-stored
`@component` values are included with their qualified name, primitive field
schema, and values; PyBevy restores them when the same component class schema
is registered. Custom resources and components that store Python objects are
skipped, and `from_world()` emits one `UserWarning` naming the skipped types
that were present. Native ECS types without Bevy's required `ReflectComponent`
or `ReflectResource` registration are also outside the format.

`DynamicWorld.serialize()` takes a live `World` because Bevy needs a type
registry to encode the snapshot and PyBevy does not expose `TypeRegistry` as a
Python value. The wrapper-component envelope is a PyBevy extension to Bevy's
scene format; plain Rust Bevy can consume the engine-native reflected entries,
while restoring Python components requires PyBevy and matching class schemas.

These APIs require exclusive `World` access. Do not retain an injected `World`
parameter after its callback or system finishes.
