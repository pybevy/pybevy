"""This example shows how you can know when a Component has been removed.

When a Component is removed from an Entity, all observers with a Remove trigger
for that Component will be notified. These observers will be called immediately
after the Component is removed.
"""

from pybevy.prelude import *


@component
class MyComponent(Component):
    """Component that will be removed after two seconds."""



def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    commands.spawn(Camera2d())
    commands.spawn(Sprite.from_image(asset_server.load_image("icon.png")), MyComponent())


def remove_component(
    time: Res[Time], commands: Commands, query: Query[Entity, With[MyComponent]]
) -> None:
    """Remove the component after two seconds."""
    if time.elapsed_secs() > 2.0:
        entities = list(query)
        if entities:
            entity = entities[0]
            if entity is not None:
                commands.entity(entity).remove(MyComponent)


def react_on_removal(trigger: On[Remove, MyComponent], query: Query[Mut[Sprite]]) -> None:
    """React to component removal by changing sprite color."""
    entity = trigger.entity()
    if entity is not None:
        sprite = query.get(entity)
        if sprite is not None:
            sprite.color = Color.srgb(0.5, 1.0, 1.0)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, remove_component)
        .add_observer(react_on_removal)
    )


if __name__ == "__main__":
    main().run()
