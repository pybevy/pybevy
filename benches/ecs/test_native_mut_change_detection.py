"""Benchmarks for native component access through mutable queries."""

from collections.abc import Callable

import pytest
from pytest_benchmark.fixture import BenchmarkFixture

from pybevy.app import App, ScheduleRunnerPlugin, Startup, Update
from pybevy.ecs import Commands, Mut, Query
from pybevy.transform import Transform

ENTITY_COUNTS = [1000, 10000]


def _benchmark_app(
    entity_count: int,
    system: Callable[..., None],
) -> App:
    def setup(commands: Commands) -> None:
        for index in range(entity_count):
            commands.spawn(Transform.from_xyz(float(index), 1.0, 2.0))

    app = App().add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, system)
    app.initialize()
    app.update()
    return app


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_native_immutable_nested_read(
    benchmark: BenchmarkFixture,
    entity_count: int,
) -> None:
    """Baseline nested-field reads through Query[Transform]."""
    visited = [0]

    def access(query: Query[Transform]) -> None:
        visited[0] = 0
        for transform in query:
            _ = transform.translation.x
            visited[0] += 1

    app = _benchmark_app(entity_count, access)
    benchmark(app.update)
    assert visited[0] == entity_count


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_native_mut_nested_read(
    benchmark: BenchmarkFixture,
    entity_count: int,
) -> None:
    """Nested-field reads through Query[Mut[Transform]]."""
    visited = [0]

    def access(query: Query[Mut[Transform]]) -> None:
        visited[0] = 0
        for transform in query:
            _ = transform.translation.x
            visited[0] += 1

    app = _benchmark_app(entity_count, access)
    benchmark(app.update)
    assert visited[0] == entity_count


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_native_mut_single_nested_write(
    benchmark: BenchmarkFixture,
    entity_count: int,
) -> None:
    """One nested-field write per native mutable component."""
    visited = [0]

    def access(query: Query[Mut[Transform]]) -> None:
        visited[0] = 0
        for transform in query:
            transform.translation.x += 1.0
            visited[0] += 1

    app = _benchmark_app(entity_count, access)
    benchmark(app.update)
    assert visited[0] == entity_count


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_native_mut_repeated_nested_write(
    benchmark: BenchmarkFixture,
    entity_count: int,
) -> None:
    """Three nested-field writes per native mutable component."""
    visited = [0]

    def access(query: Query[Mut[Transform]]) -> None:
        visited[0] = 0
        for transform in query:
            translation = transform.translation
            translation.x += 1.0
            translation.y += 1.0
            translation.z += 1.0
            visited[0] += 1

    app = _benchmark_app(entity_count, access)
    benchmark(app.update)
    assert visited[0] == entity_count
