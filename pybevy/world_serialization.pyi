from pybevy.app import App, Plugin
from pybevy.assets import Asset, Handle
from pybevy.ecs import Component, Entity, Message, Resource

class WorldSerializationPlugin(Plugin):
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class WorldAsset(Asset):
    pass

class DynamicWorld(Asset):
    """A collection of serializable resources and dynamic entities.

    DynamicWorld can be loaded from .scn.ron files and used with DynamicWorldRoot
    to spawn serialized world data.
    """

class InstanceId:
    """Unique id identifying a spawned world instance."""

    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class WorldAssetRoot(Component):
    def __init__(self, handle: Handle[WorldAsset]) -> None: ...
    def handle(self) -> Handle[WorldAsset]: ...

class DynamicWorldRoot(Component):
    """Component that spawns a DynamicWorld as a child of the entity.

    Once spawned, the entity will have a WorldInstance component.
    """

    def __init__(self, handle: Handle[DynamicWorld]) -> None: ...
    def handle(self) -> Handle[DynamicWorld]: ...

class WorldInstanceReady(Message):
    """Message sent when a world instance is fully loaded.

    Bridged from Bevy's EntityEvent system to PyBevy's Message system.
    Use MessageReader[WorldInstanceReady] to receive notifications when instances finish loading.

    Note: Unlike Bevy's native EntityEvent, this is broadcast to all systems
    (not entity-targeted), which is suitable for most loading use cases.

    Example:
        ```python
        app.add_message(WorldInstanceReady)

        def on_instance_ready(reader: MessageReader[WorldInstanceReady]) -> None:
            for event in reader:
                print(f"World instance loaded on entity: {event.entity}")

        app.add_systems(Update, on_instance_ready)
        ```
    """

    entity: Entity
    instance_id: InstanceId

    def __eq__(self, other: object) -> bool: ...

class WorldInstanceSpawner(Resource):
    """Handles spawning and despawning world instances in the world.

    Use the deferred methods (spawn, despawn) in systems - they queue the work
    to be processed by the world_instance_spawner_system.
    """

    def spawn_dynamic(self, id: Handle[DynamicWorld]) -> InstanceId:
        """Schedule the spawn of a new instance of the provided dynamic world.

        The instance will be spawned when the world_instance_spawner_system runs.
        Returns the InstanceId for tracking the spawned instance.
        """

    def spawn_dynamic_as_child(
        self, id: Handle[DynamicWorld], parent: Entity
    ) -> InstanceId:
        """Schedule the spawn of a new instance of the provided dynamic world as a child of parent.

        The instance will be spawned when the world_instance_spawner_system runs.
        Returns the InstanceId for tracking the spawned instance.
        """

    def spawn(self, id: Handle[WorldAsset]) -> InstanceId:
        """Schedule the spawn of a new instance of the provided world asset.

        The instance will be spawned when the world_instance_spawner_system runs.
        Returns the InstanceId for tracking the spawned instance.
        """

    def spawn_as_child(self, id: Handle[WorldAsset], parent: Entity) -> InstanceId:
        """Schedule the spawn of a new instance of the provided world asset as a child of parent.

        The instance will be spawned when the world_instance_spawner_system runs.
        Returns the InstanceId for tracking the spawned instance.
        """

    def despawn(self, id: Handle[WorldAsset]) -> None:
        """Schedule the despawn of all instances of the provided world asset."""

    def despawn_dynamic(self, id: Handle[DynamicWorld]) -> None:
        """Schedule the despawn of all instances of the provided dynamic world."""

    def despawn_instance(self, instance_id: InstanceId) -> None:
        """Schedule the despawn of a world instance, removing all its entities from the world."""

    def unregister_instance(self, instance_id: InstanceId) -> None:
        """Unregister a world instance without despawning its entities.

        This removes the instance tracking but leaves spawned entities in the world.
        """

    def instance_is_ready(self, instance_id: InstanceId) -> bool:
        """Check if a world instance is ready (has been spawned and entities are available)."""

    def iter_instance_entities(self, instance_id: InstanceId) -> list[Entity] | None:
        """Get the entities in an instance, once it's spawned.

        Returns None if the instance hasn't been spawned yet.
        """
