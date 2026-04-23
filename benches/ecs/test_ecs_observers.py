"""Benchmarks for Observer system performance."""

import pytest

from pybevy.app import App, RunMode, ScheduleRunnerPlugin, Startup, Update
from pybevy.ecs import Add, Commands, Despawn, Insert, On, Remove, World
from pybevy.transform import Transform


@pytest.mark.benchmark(group="observer-many-observers")
def test_performance_many_observers_same_event(benchmark) -> None:
    """Test performance with many observers on the same event type."""
    num_observers = 100

    def create_and_trigger():
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))

        for i in range(num_observers):
            exec(
                f"""
def on_add_{i}(trigger: On[Add, Transform]) -> None:
    _ = trigger.entity()
app.add_observer(on_add_{i})
""",
                {"app": app, "On": On, "Add": Add, "Transform": Transform},
            )

        def spawn_entity(commands: Commands) -> None:
            commands.spawn(Transform.from_xyz(1.0, 2.0, 3.0))

        app.add_systems(Update, spawn_entity)
        app.initialize()
        app.update()

    benchmark(create_and_trigger)


@pytest.mark.benchmark(group="observer-many-entities")
def test_performance_many_entities_lifecycle_events(benchmark) -> None:
    """Test performance of lifecycle events with many entities."""
    num_entities = 1000

    def create_and_trigger():
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))

        call_count = []

        def on_add(trigger: On[Add, Transform]) -> None:
            call_count.append(1)

        app.add_observer(on_add)

        def spawn_entities(commands: Commands) -> None:
            for i in range(num_entities):
                commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

        app.add_systems(Update, spawn_entities)
        app.initialize()
        app.update()

        assert len(call_count) == num_entities

    benchmark(create_and_trigger)


@pytest.mark.benchmark(group="observer-lifecycle-overhead")
def test_performance_lifecycle_event_overhead(benchmark) -> None:
    """Test overhead of lifecycle event triggering vs direct spawning."""
    num_entities = 100

    def spawn_with_observers():
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))

        def on_add(trigger: On[Add, Transform]) -> None:
            pass

        app.add_observer(on_add)

        def spawn_entities(commands: Commands) -> None:
            for i in range(num_entities):
                commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

        app.add_systems(Update, spawn_entities)
        app.initialize()
        app.update()

    benchmark(spawn_with_observers)


@pytest.mark.benchmark(group="observer-insert-remove")
def test_performance_insert_remove_lifecycle(benchmark) -> None:
    """Test performance of Insert and Remove lifecycle events."""
    num_operations = 100

    def insert_remove_cycle():
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))

        insert_count = []
        remove_count = []

        def on_insert(trigger: On[Insert, Transform]) -> None:
            insert_count.append(1)

        def on_remove(trigger: On[Remove, Transform]) -> None:
            remove_count.append(1)

        app.add_observer(on_insert)
        app.add_observer(on_remove)

        entity_id = None

        def setup(commands: Commands) -> None:
            nonlocal entity_id
            e = commands.spawn_empty()
            entity_id = e.id()

        def insert_remove(commands: Commands) -> None:
            assert entity_id is not None
            for _ in range(num_operations):
                commands.entity(entity_id).insert(Transform.from_xyz(1.0, 2.0, 3.0))
                commands.entity(entity_id).remove(Transform)

        app.add_systems(Startup, setup)
        app.add_systems(Update, insert_remove)
        app.initialize()
        app.update()

        assert len(insert_count) == num_operations
        assert len(remove_count) == num_operations

    benchmark(insert_remove_cycle)


@pytest.mark.benchmark(group="observer-filtered")
def test_performance_filtered_observers(benchmark) -> None:
    """Test performance of observers with bundle filters."""
    from pybevy.decorators import component
    from pybevy.ecs import Component

    @component
    class Marker(Component):
        """Marker component for filtering."""


    num_entities = 100

    def spawn_filtered():
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))

        filtered_count = []

        def on_add_filtered(trigger: On[Add, Transform]) -> None:
            filtered_count.append(1)

        app.add_observer(on_add_filtered)

        def spawn_entities(commands: Commands) -> None:
            for i in range(num_entities):
                if i % 2 == 0:
                    commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0), Marker())
                else:
                    commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

        app.add_systems(Update, spawn_entities)
        app.initialize()
        app.update()

        assert len(filtered_count) == num_entities

    benchmark(spawn_filtered)


@pytest.mark.benchmark(group="observer-despawn")
def test_performance_despawn_lifecycle(benchmark) -> None:
    """Test performance of Despawn lifecycle events."""
    num_entities = 100

    def spawn_and_despawn():
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))

        despawn_count = []

        def on_despawn(trigger: On[Despawn, Transform]) -> None:
            despawn_count.append(1)

        app.add_observer(on_despawn)

        entity_ids = []

        def spawn_entities(world: World) -> None:
            for i in range(num_entities):
                e = world.spawn(Transform.from_xyz(float(i), 0.0, 0.0))
                entity_ids.append(e.id())

        def despawn_entities(world: World) -> None:
            for entity_id in entity_ids:
                world.despawn(entity_id)

        app.add_systems(Startup, spawn_entities)
        app.add_systems(Update, despawn_entities)
        app.initialize()
        app.update()

        assert len(despawn_count) == num_entities

    benchmark(spawn_and_despawn)


@pytest.mark.benchmark(group="observer-comparison")
def test_performance_no_observers_baseline(benchmark) -> None:
    """Baseline performance test: spawning entities with no observers."""
    num_entities = 1000

    def spawn_no_observers():
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))

        def spawn_entities(commands: Commands) -> None:
            for i in range(num_entities):
                commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

        app.add_systems(Update, spawn_entities)
        app.initialize()
        app.update()

    benchmark(spawn_no_observers)
