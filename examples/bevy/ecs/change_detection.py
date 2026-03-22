"""Demonstrates the concept of change detection in ECS.

Demonstrates:
- Component modification tracking
- Resource modification tracking
- How change detection reduces unnecessary system work

Note: PyBevy doesn't yet support Changed[T] and Added[T] filters for custom
components due to a registration bug. This example demonstrates the concept
using manual change tracking as a workaround.

For built-in components like Transform, PointLight, etc., Changed[T] and
Added[T] filters work correctly.
"""

import random

from pybevy.prelude import *


@component
class MyComponent(Component):
    """Custom component for demonstrating change detection concept."""



@resource
class MyResource(Resource):
    """Custom resource with change tracking."""

    def __init__(self, value: float = 0.0, changed_this_frame: bool = False):
        self.value = value
        self.changed_this_frame = changed_this_frame
        self.previous_value = value


def setup(commands: Commands) -> None:
    """Spawn an entity with MyComponent and insert MyResource."""
    commands.spawn(MyComponent())
    commands.insert_resource(MyResource(0.0))
    print("Setup: Spawned entity with MyComponent and inserted MyResource")


def change_component(
    time: Res[Time], query: Query[tuple[Entity, Mut[MyComponent]]]
) -> None:
    """Randomly modify MyComponent.

    In Bevy Rust, modifying a component through Mut<T> automatically marks
    it as changed. Change detection then allows other systems to react only
    when components have actually been modified.
    """
    for entity, _component in query:
        # Randomly modify the component (10% chance)
        if random.random() < 0.1:
            new_value = round(time.elapsed_secs())
            print(f"  [change_component] Modified component on {entity} (tick={new_value})")
            # In Bevy, any access to Mut<T> marks it as changed
            # Other systems could use Query[T, Changed[T]] to detect this


def change_component_2(
    time: Res[Time], query: Query[tuple[Entity, Mut[MyComponent]]]
) -> None:
    """Second system that might modify MyComponent.

    Having multiple systems modify the same component demonstrates why
    change detection is useful - it helps identify WHERE changes occur.
    """
    for entity, _component in query:
        # Randomly modify the component (5% chance)
        if random.random() < 0.05:
            new_value = round(time.elapsed_secs())
            print(f"  [change_component_2] Modified component on {entity} (tick={new_value})")


def change_resource(time: Res[Time], my_resource: ResMut[MyResource]) -> None:
    """Randomly modify MyResource and track changes.

    Demonstrates manual change tracking since Changed<T> filter doesn't
    work with custom resources yet.
    """
    # Reset change flag each frame
    my_resource.changed_this_frame = False

    if random.random() < 0.15:
        new_value = round(time.elapsed_secs())
        if new_value != my_resource.value:
            my_resource.previous_value = my_resource.value
            my_resource.value = new_value
            my_resource.changed_this_frame = True
            print(
                f"  [change_resource] Resource changed: {my_resource.previous_value} -> {my_resource.value}"
            )


def detect_resource_changes(my_resource: Res[MyResource]) -> None:
    """React to resource changes using manual tracking.

    In Bevy Rust, you would check my_resource.is_changed() to detect
    modifications. PyBevy doesn't support this yet, so we use a manual
    flag as a workaround.

    This demonstrates the value of change detection: this system only
    does work when the resource has actually changed.
    """
    if my_resource.changed_this_frame:
        print(f"    [detect_resource_changes] Detected change! value={my_resource.value}")


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                change_component,
                change_component_2,
                change_resource,
                detect_resource_changes,
            ),
        )
    )


if __name__ == "__main__":
    print("Change Detection Example")
    print("=" * 60)
    print("Watch for component and resource modifications.")
    print("In Bevy Rust, you'd use Changed[T] and Added[T] filters")
    print("to react only when components change.")
    print("=" * 60)
    main().run()
