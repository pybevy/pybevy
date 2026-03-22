"""Benchmarks for Query iteration performance."""

import pytest
from pytest_benchmark.fixture import BenchmarkFixture

from pybevy.app import App, ScheduleRunnerPlugin, Startup, Update
from pybevy.ecs import Commands, Entity, Mut, Query
from pybevy.math import Quat, Vec3
from pybevy.transform import Transform


def _bench_query_minimal_iteration(entity_count: int) -> None:
    """Helper to benchmark minimal query iteration overhead."""

    def setup(commands: Commands) -> None:
        for _ in range(entity_count):
            commands.spawn(Transform())

    def minimal_iteration(query: Query[Transform]) -> None:
        for _ in query:
            pass

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, minimal_iteration)
    app.initialize()
    app.update()


def _bench_query_count(entity_count: int) -> int:
    """Helper to benchmark query iteration with counting."""

    def setup(commands: Commands) -> None:
        for _ in range(entity_count):
            commands.spawn(Transform())

    count = [0]

    def count_iteration(query: Query[Transform]) -> None:
        count[0] = 0
        for _ in query:
            count[0] += 1

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, count_iteration)
    app.initialize()
    app.update()

    return count[0]


def _bench_query_read_translation(entity_count: int) -> None:
    """Helper to benchmark reading translation."""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

    total = [0.0]

    def read_translations(query: Query[Transform]) -> None:
        total[0] = 0.0
        for transform in query:
            total[0] += transform.translation.x

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, read_translations)
    app.initialize()
    app.update()


def _bench_query_write_translation(entity_count: int) -> None:
    """Helper to benchmark writing translation."""

    def setup(commands: Commands) -> None:
        for _ in range(entity_count):
            commands.spawn(Transform())

    def write_translations(query: Query[Transform]) -> None:
        for i, transform in enumerate(query):
            transform.translation = Vec3(float(i), 1.0, 2.0)

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, write_translations)
    app.initialize()
    app.update()


def _bench_query_rotate(entity_count: int, axis: str) -> None:
    """Helper to benchmark rotation operations."""

    def setup(commands: Commands) -> None:
        for _ in range(entity_count):
            commands.spawn(Transform())

    def rotate_transforms(query: Query[Transform]) -> None:
        for transform in query:
            if axis == "x":
                transform.rotate_x(0.01)
            elif axis == "y":
                transform.rotate_y(0.01)
            elif axis == "z":
                transform.rotate_z(0.01)
            elif axis == "local_x":
                transform.rotate_local_x(0.01)
            elif axis == "quat":
                transform.rotate(Quat.from_rotation_y(0.01))

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, rotate_transforms)
    app.initialize()
    app.update()


def _bench_query_mutate_transform(entity_count: int) -> int:
    """Helper to benchmark Transform mutation via query."""
    addition = Vec3(0.1, 0.2, 0.3)

    def setup(commands: Commands) -> None:
        for _ in range(entity_count):
            commands.spawn(Transform())

    count = [0]

    def query_system(query: Query[Mut[Transform]]) -> None:
        count[0] = 0
        for transform in query:
            transform.translation += addition
            count[0] += 1

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, query_system)
    app.initialize()
    app.update()

    return count[0]


ENTITY_COUNTS = [1000, 10000]


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_iteration_minimal(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark minimal query iteration overhead."""
    benchmark(_bench_query_minimal_iteration, entity_count)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_iteration_count(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark query iteration with counting."""
    count = benchmark(_bench_query_count, entity_count)
    assert count == entity_count


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_translation_read(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark reading translation from transforms."""
    benchmark(_bench_query_read_translation, entity_count)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_translation_write(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark writing translation to transforms."""
    benchmark(_bench_query_write_translation, entity_count)


@pytest.mark.parametrize("entity_count", [1000, 10000])
@pytest.mark.parametrize("axis", ["x", "y", "z", "local_x", "quat"])
def test_rotate(
    benchmark: BenchmarkFixture, entity_count: int, axis: str
) -> None:
    """Benchmark rotation operations."""
    benchmark(_bench_query_rotate, entity_count, axis)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_mutate_transform(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark Transform mutation via Mut[Transform]."""
    count = benchmark(_bench_query_mutate_transform, entity_count)
    assert count == entity_count


def test_look_at_10000(benchmark: BenchmarkFixture) -> None:
    """Benchmark look_at operation (10k entities)."""
    entity_count = 10000

    def setup(commands: Commands) -> None:
        for _ in range(entity_count):
            commands.spawn(Transform())

    target = Vec3(1.0, 0.0, 0.0)
    up = Vec3.Y

    def look_at_transforms(query: Query[Transform]) -> None:
        for transform in query:
            transform.look_at(target, up)

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, look_at_transforms)
    app.initialize()
    app.update()

    benchmark(app.update)


def test_forward_direction_10000(benchmark: BenchmarkFixture) -> None:
    """Benchmark reading forward direction (10k entities)."""
    entity_count = 10000

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            t = Transform.from_xyz(float(i), 0.0, 0.0)
            t.rotate_y(0.01 * i)
            commands.spawn(t)

    total = [Vec3.ZERO]

    def read_directions(query: Query[Transform]) -> None:
        total[0] = Vec3.ZERO
        for transform in query:
            fwd = transform.forward()
            total[0] = Vec3(
                total[0].x + fwd.x,
                total[0].y + fwd.y,
                total[0].z + fwd.z,
            )

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, read_directions)
    app.initialize()
    app.update()

    benchmark(app.update)


def test_transform_point_10000(benchmark: BenchmarkFixture) -> None:
    """Benchmark transform_point operation (10k entities)."""
    entity_count = 10000

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            t = Transform.from_xyz(float(i), 0.0, 0.0)
            t.rotate_y(0.01 * i)
            commands.spawn(t)

    point = Vec3(1.0, 2.0, 3.0)

    def transform_points(query: Query[Transform]) -> None:
        for transform in query:
            _ = transform.transform_point(point)

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, transform_points)
    app.initialize()
    app.update()

    benchmark(app.update)


def test_mixed_operations_10000(benchmark: BenchmarkFixture) -> None:
    """Benchmark mixed read/write operations (10k entities)."""
    entity_count = 10000

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

    def mixed_operations(query: Query[Transform]) -> None:
        for i, transform in enumerate(query):
            pos = transform.translation

            if i % 2 == 0:
                transform.rotate_x(0.01)
            else:
                transform.rotate_y(0.01)

            transform.translation = Vec3(pos.x + 0.1, pos.y, pos.z)

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, mixed_operations)
    app.initialize()
    app.update()

    benchmark(app.update)


def test_multi_property_access_10000(benchmark: BenchmarkFixture) -> None:
    """Benchmark multiple property access overhead (10k entities)."""
    entity_count = 10000

    def setup(commands: Commands) -> None:
        for _ in range(entity_count):
            commands.spawn(Transform())

    def access_properties(query: Query[Transform]) -> None:
        for transform in query:
            _ = transform.translation
            _ = transform.rotation
            _ = transform.scale
            _ = transform.translation.x
            _ = transform.rotation.w

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, access_properties)
    app.initialize()
    app.update()

    benchmark(app.update)


def test_entity_transform_query_10000(benchmark: BenchmarkFixture) -> None:
    """Benchmark multi-component query (Entity + Transform, 10k entities)."""
    entity_count = 10000

    def setup(commands: Commands) -> None:
        for _ in range(entity_count):
            commands.spawn(Transform())

    count = [0]

    def query_system(query: Query[tuple[Entity, Transform]]) -> None:
        count[0] = 0
        for _entity, _transform in query:
            count[0] += 1

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, query_system)
    app.initialize()
    app.update()

    benchmark(app.update)
    assert count[0] == entity_count
