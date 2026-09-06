# Bevy Interoperability

PyBevy runs on Bevy's ECS and can exchange reflected world data with a Rust
Bevy application through Bevy's native `.scn.ron` format. Use this for scene
data, reusable world fragments, debugging snapshots, or moving authored state
between Python and Rust implementations that target the same Bevy version.

## Capture a live PyBevy world

`DynamicWorld.from_world()` creates an owned reflection snapshot. Serializing
that snapshot produces the same RON shape used by Bevy's
`bevy_world_serialization` crate:

```python
from pybevy.ecs import World
from pybevy.world_serialization import DynamicWorld


def capture(world: World) -> str:
    snapshot = DynamicWorld.from_world(world)
    return snapshot.serialize(world)
```

The corresponding Rust Bevy flow is:

```rust
fn capture(world: &World) -> Result<String, ron::Error> {
    let snapshot = DynamicWorld::from_world(world);
    let registry = world.resource::<AppTypeRegistry>().read();
    snapshot.serialize(&registry)
}
```

Rust accepts a `TypeRegistry` directly. PyBevy accepts its owning `World`
because the registry is not exposed as a Python value.

## Load and spawn `.scn.ron`

Add `WorldSerializationPlugin`, load the file as `DynamicWorld`, and spawn a
`DynamicWorldRoot`:

```python
from pybevy.prelude import *
from pybevy.world_serialization import (
    DynamicWorld,
    DynamicWorldRoot,
    WorldSerializationPlugin,
)


def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    scene = asset_server.load("levels/tutorial.scn.ron", DynamicWorld)
    commands.spawn(DynamicWorldRoot(scene))


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins, WorldSerializationPlugin)
        .add_systems(Startup, setup)
    )
```

Rust Bevy loads and spawns the same asset as
`Handle<DynamicWorld>` plus `DynamicWorldRoot`.

## Reflection boundary

The format contains only ECS values available through `AppTypeRegistry` with
`ReflectComponent` or `ReflectResource` metadata. PyBevy adds reflection for
wrapper-stored `@component` values: their qualified class name, exact primitive
field schema, and owned values are stored in a PyBevy envelope inside the Bevy
scene. Loading through PyBevy materializes those entries back into their real
dynamic component IDs when a matching class schema is registered.

That envelope is a PyBevy interoperability extension, not a native Rust type
for each Python class. A Rust application that wants to restore those dynamic
components must embed PyBevy and register the matching Python component
classes. Engine-native reflected components remain directly interoperable with
ordinary Bevy applications.

Custom resources and components that store Python objects are skipped;
`DynamicWorld.from_world()` emits a `UserWarning` naming the skipped types that
were present.

Systems, Python callables, GPU state, and other runtime-only state are not part
of a `DynamicWorld`. Treat it as reflected scene/world data rather than a
complete process snapshot. The reader must also register compatible types, and
both applications should target the same Bevy version because scene schemas
can change between Bevy releases.

For embedding Python systems directly in a Rust Bevy application instead of
exchanging scene data, see [Native Plugin Usage](native-plugin.md).
