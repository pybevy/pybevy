from pybevy.app import App, Plugin
from pybevy.assets import Asset, Handle
from pybevy.ecs import Component, Entity, Message, Resource

class ScenePlugin(Plugin):
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class Scene(Asset):
    pass

class DynamicScene(Asset):
    """A collection of serializable resources and dynamic entities.

    DynamicScene can be loaded from .scn.ron files and used with DynamicSceneRoot
    to spawn serialized scene data.
    """

class InstanceId:
    """Unique id identifying a scene instance."""

    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class SceneRoot(Component):
    def __init__(self, scene: Handle[Scene]) -> None: ...
    def handle(self) -> Handle[Scene]: ...

class DynamicSceneRoot(Component):
    """Component that spawns a DynamicScene as a child of the entity.

    Once spawned, the entity will have a SceneInstance component.
    """

    def __init__(self, scene: Handle[DynamicScene]) -> None: ...
    def handle(self) -> Handle[DynamicScene]: ...

class SceneInstanceReady(Message):
    """Message sent when a scene instance is fully loaded.

    Bridged from Bevy's EntityEvent system to PyBevy's Message system.
    Use MessageReader[SceneInstanceReady] to receive notifications when scenes finish loading.

    Note: Unlike Bevy's native EntityEvent, this is broadcast to all systems
    (not entity-targeted), which is suitable for most scene loading use cases.

    Example:
        ```python
        app.add_message(SceneInstanceReady)

        def on_scene_ready(reader: MessageReader[SceneInstanceReady]) -> None:
            for event in reader:
                print(f"Scene loaded on entity: {event.entity}")

        app.add_systems(Update, on_scene_ready)
        ```
    """

    entity: Entity
    instance_id: InstanceId

    def __eq__(self, other: object) -> bool: ...

class SceneSpawner(Resource):
    """Handles spawning and despawning scenes in the world.

    Use the deferred methods (spawn, despawn) in systems - they queue the work
    to be processed by the scene_spawner_system.
    """

    def spawn_dynamic(self, id: Handle[DynamicScene]) -> InstanceId:
        """Schedule the spawn of a new instance of the provided dynamic scene.

        The scene will be spawned when the scene_spawner_system runs.
        Returns the InstanceId for tracking the spawned instance.
        """

    def spawn_dynamic_as_child(
        self, id: Handle[DynamicScene], parent: Entity
    ) -> InstanceId:
        """Schedule the spawn of a new instance of the provided dynamic scene as a child of parent.

        The scene will be spawned when the scene_spawner_system runs.
        Returns the InstanceId for tracking the spawned instance.
        """

    def spawn(self, id: Handle[Scene]) -> InstanceId:
        """Schedule the spawn of a new instance of the provided scene.

        The scene will be spawned when the scene_spawner_system runs.
        Returns the InstanceId for tracking the spawned instance.
        """

    def spawn_as_child(self, id: Handle[Scene], parent: Entity) -> InstanceId:
        """Schedule the spawn of a new instance of the provided scene as a child of parent.

        The scene will be spawned when the scene_spawner_system runs.
        Returns the InstanceId for tracking the spawned instance.
        """

    def despawn(self, id: Handle[Scene]) -> None:
        """Schedule the despawn of all instances of the provided scene."""

    def despawn_dynamic(self, id: Handle[DynamicScene]) -> None:
        """Schedule the despawn of all instances of the provided dynamic scene."""

    def despawn_instance(self, instance_id: InstanceId) -> None:
        """Schedule the despawn of a scene instance, removing all its entities from the world."""

    def unregister_instance(self, instance_id: InstanceId) -> None:
        """Unregister a scene instance without despawning its entities.

        This removes the instance tracking but leaves spawned entities in the world.
        """

    def instance_is_ready(self, instance_id: InstanceId) -> bool:
        """Check if a scene instance is ready (has been spawned and entities are available)."""

    def iter_instance_entities(self, instance_id: InstanceId) -> list[Entity] | None:
        """Get the entities in an instance, once it's spawned.

        Returns None if the instance hasn't been spawned yet.
        """
