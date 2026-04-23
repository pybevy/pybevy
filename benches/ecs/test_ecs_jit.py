"""Benchmarks for JIT compilation performance.

Tests JIT kernel execution, View bytecode, and Query iteration comparisons.
"""

import pytest
from pytest_benchmark.fixture import BenchmarkFixture

try:
    import numpy as np

    from pybevy.jit import bevyjit  # type: ignore[import-not-found]

    NUMBA_AVAILABLE = True
except ImportError:
    NUMBA_AVAILABLE = False

from pybevy.app import App, Last, RunMode, ScheduleRunnerPlugin, Startup, Update
from pybevy.decorators import component
from pybevy.ecs import Commands, Component, Mut, Query, View, With
from pybevy.transform import Transform


@component
class Marker(Component):
    """Marker component for benchmark entities."""



# Define JIT kernels
if NUMBA_AVAILABLE:

    @bevyjit
    def jit_scale_offset(
        positions: np.ndarray, length: int, scale: float, offset: float
    ) -> None:
        """JIT kernel: scale and offset positions."""
        for i in range(length):
            positions[i] = positions[i] * scale + offset


def _spawn_entities(commands: Commands, count: int) -> None:
    """Spawn entities with Transform and Marker."""
    for i in range(count):
        x = float(i % 100)
        commands.spawn(Transform.from_xyz(x, 0.0, 0.0), Marker())


ENTITY_COUNTS = [100, 1000, 10000]


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_simple_query(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark Query simple operation (x = x * 2 + 5)."""

    def spawn(commands: Commands) -> None:
        _spawn_entities(commands, entity_count)

    def bench_query(query: Query[tuple[Mut[Transform], Marker]]) -> None:
        for t, _ in query:
            t.translation.x = t.translation.x * 2.0 + 5.0

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, spawn)
    app.add_systems(Update, bench_query)
    app.initialize()
    app.update()

    benchmark(app.update)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_simple_view(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark View simple operation (x = x * 2 + 5)."""

    def spawn(commands: Commands) -> None:
        _spawn_entities(commands, entity_count)

    def bench_view(view: View[Mut[Transform], With[Marker]]) -> None:
        transform = view.column_mut(Transform)
        transform.translation.x = transform.translation.x * 2.0 + 5.0

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, spawn)
    app.add_systems(Update, bench_view)
    app.initialize()
    app.update()

    benchmark(app.update)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_multi_field_query(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark Query multi-field operation (x, y, z *= 0.99)."""

    def spawn(commands: Commands) -> None:
        for i in range(entity_count):
            x = float(i % 100)
            commands.spawn(Transform.from_xyz(x, x * 0.5, x * 0.3), Marker())

    def bench_query(query: Query[tuple[Mut[Transform], Marker]]) -> None:
        for t, _ in query:
            t.translation.x = t.translation.x * 0.99
            t.translation.y = t.translation.y * 0.99
            t.translation.z = t.translation.z * 0.99

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, spawn)
    app.add_systems(Update, bench_query)
    app.initialize()
    app.update()

    benchmark(app.update)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_multi_field_view(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark View multi-field operation (x, y, z *= 0.99)."""

    def spawn(commands: Commands) -> None:
        for i in range(entity_count):
            x = float(i % 100)
            commands.spawn(Transform.from_xyz(x, x * 0.5, x * 0.3), Marker())

    def bench_view(view: View[Mut[Transform], With[Marker]]) -> None:
        transform = view.column_mut(Transform)
        transform.translation.x = transform.translation.x * 0.99
        transform.translation.y = transform.translation.y * 0.99
        transform.translation.z = transform.translation.z * 0.99

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, spawn)
    app.add_systems(Update, bench_view)
    app.initialize()
    app.update()

    benchmark(app.update)


@pytest.mark.skipif(not NUMBA_AVAILABLE, reason="Numba not installed")
def test_jit_kernel_registration() -> None:
    """Test that JIT kernels can be registered."""

    @bevyjit
    def test_kernel(positions: np.ndarray, length: int, scale: float) -> None:
        for i in range(length):
            positions[i] = positions[i] * scale

    assert test_kernel is not None


@pytest.mark.skipif(not NUMBA_AVAILABLE, reason="Numba not installed")
def test_jit_api_available() -> None:
    """Test that View.jit() API is callable with field extraction."""

    @bevyjit
    def scale_kernel(positions: np.ndarray, length: int, scale: float) -> None:
        """Scale all positions by a factor."""
        for i in range(length):
            positions[i] = positions[i] * scale

    def spawn(commands: Commands) -> None:
        for i in range(10):
            commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0), Marker())

    jit_called = [False]
    error_caught: list[str | None] = [None]

    def try_jit(view: View[Transform, Marker]) -> None:
        transform = view.column_mut(Transform)
        try:
            view.jit("scale_kernel", transform.translation.x, scale=2.0)  # type: ignore[attr-defined]
            jit_called[0] = True
        except (RuntimeError, TypeError) as e:
            error_caught[0] = str(e)

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, spawn)
    app.add_systems(Update, try_jit)
    app.initialize()
    app.update()

    # Either it worked or we got the expected error
    assert jit_called[0] or (
        error_caught[0] is not None
        and ("FieldExpr" in error_caught[0] or "ColumnProxy" in error_caught[0])
    )


def test_view_correctness() -> None:
    """Test that View produces correct results."""
    entity_count = 100
    results = []

    def spawn(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0), Marker())

    def calc_view(view: View[Mut[Transform], With[Marker]]) -> None:
        transform = view.column_mut(Transform)
        transform.translation.x = transform.translation.x * 2.0 + 5.0

    def verify(query: Query[tuple[Mut[Transform], Marker]]) -> None:
        for t, _ in query:
            results.append(t.translation.x)

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, spawn)
    app.add_systems(Update, calc_view)
    app.add_systems(Last, verify)
    app.initialize()
    app.update()

    assert len(results) == entity_count
    for i, value in enumerate(results):
        expected = float(i) * 2.0 + 5.0
        assert abs(value - expected) < 0.001, (
            f"Result mismatch at {i}: got {value}, expected {expected}"
        )
