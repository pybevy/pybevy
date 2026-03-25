"""Entity hierarchy with parent-child relationships.

Demonstrates:
- Creating parent-child relationships with ChildOf component
- Automatic Children component management
- Traversing entity hierarchies
- Iterating over children
- Transform propagation in hierarchies
"""

from pybevy.prelude import *


@component(storage="python")
class Parent(Component):
    """Marker for parent entities."""
    name: str

    def __init__(self, name: str):
        self.name = name


@component(storage="python")
class Child(Component):
    """Marker for child entities."""
    name: str

    def __init__(self, name: str):
        self.name = name


def setup(commands: Commands) -> None:
    """Set up entity hierarchy."""
    # Create a parent entity
    parent = commands.spawn(Transform.from_xyz(0.0, 0.0, 0.0), Parent("Root")).id()

    # Create child entities with ChildOf pointing to parent
    # The Children component will be automatically added to the parent
    child1 = commands.spawn(Transform.from_xyz(2.0, 0.0, 0.0), Child("Child1"), ChildOf(parent)).id()

    child2 = commands.spawn(Transform.from_xyz(-2.0, 0.0, 0.0), Child("Child2"), ChildOf(parent)).id()

    child3 = commands.spawn(Transform.from_xyz(0.0, 2.0, 0.0), Child("Child3"), ChildOf(parent)).id()

    # Create a grandchild (child of child1)
    grandchild = commands.spawn(Transform.from_xyz(0.0, 1.0, 0.0), Child("Grandchild"), ChildOf(child1)).id()

    print("\nHierarchy created:")
    print(f"Parent: {parent}")
    print(f"  Child1: {child1}")
    print(f"    Grandchild: {grandchild}")
    print(f"  Child2: {child2}")
    print(f"  Child3: {child3}")


def print_hierarchy(
    parents: Query[tuple[Entity, Parent, Children]],
    children_query: Query[tuple[Entity, Child]],
) -> None:
    """Print the entity hierarchy once."""
    for parent_entity, parent, children in parents:
        print("\n=== Hierarchy Info ===")
        print(f"Parent '{parent.name}' ({parent_entity}) has {len(children)} children:")

        for i, child_entity in enumerate(children):
            # Try to get child component info
            result = children_query.get(child_entity)
            if result is not None:
                _, child = result
                print(f"  [{i}] '{child.name}' ({child_entity})")
            else:
                print(f"  [{i}] Unknown entity ({child_entity})")


def animate_parent(
    time: Res[Time],
    parents: Query[Mut[Transform], With[Parent]],
) -> None:
    """Animate parent rotation - children will follow due to transform propagation."""
    for transform in parents:
        # Rotate parent around Y axis
        angle = time.elapsed_secs() * 0.5
        transform.rotation = Quat.from_rotation_y(angle)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (print_hierarchy, animate_parent))
    )


if __name__ == "__main__":
    main().run()
