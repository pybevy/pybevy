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
schema, and values. Fieldless automatic-storage components are included as
markers with an empty schema. Import the matching decorated classes before
loading; PyBevy registers them on demand in the destination World. All custom
resources and components using Python-object storage are skipped, including
fieldless markers explicitly declared with `storage="python"`.
`from_world()` warns about skipped Python values and can emit a separate warning
for reflected values that cannot be serialized. Native ECS types without Bevy's
required `ReflectComponent` or `ReflectResource` registration are also outside
the format.

`DynamicWorld.serialize()` takes a live `World` because Bevy needs a type
registry to encode the snapshot and PyBevy does not expose `TypeRegistry` as a
Python value. The wrapper-component envelope is a PyBevy extension to Bevy's
scene format; plain Rust Bevy can consume the engine-native reflected entries,
while restoring Python components requires PyBevy and matching class schemas.

Handles to code-created assets without a file path serialize as default handles,
not the asset contents. Recreate those assets and reconnect their handles after
loading, especially camera render targets. File-backed assets must remain
available at their recorded paths. Keep a strong handle in a component, resource,
or live Python variable while an asset is needed; an `AssetId` alone does not
keep it loaded.

These APIs require exclusive `World` access. Do not retain an injected `World`
parameter after its callback or system finishes.
