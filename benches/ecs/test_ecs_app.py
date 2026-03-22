"""Benchmarks for App and ECS framework overhead."""

from pytest_benchmark.fixture import BenchmarkFixture

from pybevy.app import App, MinimalPlugins, Update
from pybevy.ecs import Commands, Entity, Query, Res, World
from pybevy.time import Time
from pybevy.transform import Transform


def test_empty_app_update(benchmark: BenchmarkFixture) -> None:
    """Benchmark minimal app update with no systems."""
    app = App().add_plugins(MinimalPlugins)
    app.initialize()
    benchmark(app.update)


def test_app_update(benchmark: BenchmarkFixture) -> None:
    """Benchmark app update with empty system."""

    def update_system() -> None:
        pass

    app = App().add_plugins(MinimalPlugins).add_systems(Update, update_system)

    app.initialize()
    benchmark(app.update)


def test_app_update_with_time(benchmark: BenchmarkFixture) -> None:
    """Benchmark app update with Time resource injection."""

    def time_system(time: Res[Time]) -> None:
        pass

    app = App().add_plugins(MinimalPlugins).add_systems(Update, time_system)

    app.initialize()
    benchmark(app.update)


def test_schedule_empty(benchmark: BenchmarkFixture) -> None:
    """Benchmark schedule execution with dummy system."""

    def dummy_system() -> None:
        pass

    app = App().add_plugins(MinimalPlugins).add_systems(Update, dummy_system)
    app.update()

    benchmark(app.update)


def test_schedule_time(benchmark: BenchmarkFixture) -> None:
    """Benchmark schedule with Time parameter."""

    def dummy_system(time: Res[Time]) -> None:
        pass

    app = App().add_plugins(MinimalPlugins).add_systems(Update, dummy_system)
    app.update()

    benchmark(app.update)


def test_schedule_time_and_query(benchmark: BenchmarkFixture) -> None:
    """Benchmark schedule with Time and Query parameters."""

    def dummy_system(time: Res[Time], query: Query[Transform]) -> None:
        pass

    app = App().add_plugins(MinimalPlugins).add_systems(Update, dummy_system)
    app.update()

    benchmark(app.update)


def test_schedule_multiple_params(benchmark: BenchmarkFixture) -> None:
    """Benchmark schedule with multiple system parameters."""

    def dummy_system(
        time: Res[Time], query: Query[Transform], commands: Commands
    ) -> None:
        pass

    app = App().add_plugins(MinimalPlugins).add_systems(Update, dummy_system)
    app.update()

    benchmark(app.update)


def test_query_iter_create_empty(benchmark: BenchmarkFixture) -> None:
    """Benchmark query construction overhead."""

    def dummy_system(query: Query[Transform]) -> None:
        query.__iter__()

    app = App().add_plugins(MinimalPlugins).add_systems(Update, dummy_system)
    app.update()

    benchmark(app.update)


def test_query_loop_empty(benchmark: BenchmarkFixture) -> None:
    """Benchmark empty query loop overhead."""

    def dummy_system(query: Query[Transform]) -> None:
        for _ in query:
            pass

    app = App().add_plugins(MinimalPlugins).add_systems(Update, dummy_system)
    app.update()

    benchmark(app.update)


def test_query_iter_100_empty_entities(benchmark: BenchmarkFixture) -> None:
    """Benchmark query with 100 empty entities."""

    def dummy_system(query: Query[Entity]) -> None:
        query.__iter__()

    app = App().add_plugins(MinimalPlugins).add_systems(Update, dummy_system)

    def setup_entities(world: World) -> None:
        for _ in range(100):
            world.spawn_empty()

    app.world(setup_entities)

    app.initialize()
    app.update()

    benchmark(app.update)


def test_query_iter_100_transforms(benchmark: BenchmarkFixture) -> None:
    """Benchmark query with 100 Transform entities."""

    def dummy_system(query: Query[Transform]) -> None:
        query.__iter__()

    app = App().add_plugins(MinimalPlugins).add_systems(Update, dummy_system)

    def setup_entities(world: World) -> None:
        for _ in range(100):
            world.spawn(Transform())

    app.world(setup_entities)

    app.initialize()
    app.update()

    benchmark(app.update)


def test_query_loop_100_transforms(benchmark: BenchmarkFixture) -> None:
    """Benchmark query iteration with 100 Transform entities."""

    def dummy_system(query: Query[Transform]) -> None:
        for _ in query:
            pass

    app = App().add_plugins(MinimalPlugins).add_systems(Update, dummy_system)

    def setup_entities(world: World) -> None:
        for _ in range(100):
            world.spawn(Transform())

    app.world(setup_entities)

    app.initialize()
    app.update()

    benchmark(app.update)
