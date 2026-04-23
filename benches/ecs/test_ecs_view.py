"""Benchmarks for View API batch operations and performance comparisons."""

import pytest
from pytest_benchmark.fixture import BenchmarkFixture

numba = pytest.importorskip("numba", reason="Numba not installed")

from dataclasses import dataclass

from pybevy.app import App, RunMode, ScheduleRunnerPlugin, Startup, Update
from pybevy.decorators import component
from pybevy.ecs import Commands, Component, Mut, Query, View
from pybevy.transform import Transform


@numba.jit(nopython=True)
def numba_sum(col):
    """Pure sum - measures iteration + access overhead."""
    total = 0.0
    for i in range(len(col)):
        total += col[i]
    return total


@numba.jit(nopython=True)
def numba_physics(positions, velocities, dt):
    """Physics-like computation."""
    for i in range(len(positions)):
        vel = velocities[i]
        pos = positions[i]
        new_pos = pos + vel * dt + 0.5 * vel * vel * dt
        positions[i] = new_pos


@component
@dataclass
class Position(Component):
    x: float
    y: float
    z: float


@component
@dataclass
class Velocity(Component):
    vx: float
    vy: float
    vz: float


@component
@dataclass
class Marker(Component):
    pass


_numba_warmed_up = False


def _warmup_numba() -> None:
    """Warm up Numba JIT compilation (called lazily on first use)."""
    global _numba_warmed_up
    if _numba_warmed_up:
        return

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))

    def setup(commands: Commands) -> None:
        commands.spawn(Transform.from_xyz(1.0, 0.0, 0.0))

    def warmup(view: View[Mut[Transform]]) -> None:
        for batch in view.iter_batches():
            col = batch.column_mut(Transform)
            _ = numba_sum(col)

    app.add_systems(Startup, setup)
    app.add_systems(Update, warmup)
    app.initialize()
    app.update()
    _numba_warmed_up = True


def _setup_view_numba_sum(entity_count: int) -> App:
    """Setup app for View Numba sum benchmark."""
    _warmup_numba()

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

    def measure_view(view: View[Mut[Transform]]) -> None:
        total = 0.0
        for batch in view.iter_batches():
            col = batch.column_mut(Transform)
            total += numba_sum(col)

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, measure_view)
    app.initialize()
    app.update()
    return app


def _setup_query_sum(entity_count: int) -> App:
    """Setup app for Query sum benchmark."""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

    def measure_query(query: Query[Transform]) -> None:
        total = 0.0
        for t in query:
            total += t.translation.x

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, measure_query)
    app.initialize()
    app.update()
    return app


def _setup_view_batch_ops(entity_count: int) -> App:
    """Setup app for View batch operations benchmark."""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(
                Position(x=float(i), y=float(i * 2), z=float(i * 3)),
                Marker(),
            )

    def batch_ops(view: View[Mut[Position], Marker]) -> None:
        pos = view.column_mut(Position)
        pos.x = pos.x * 2.0 + 1.0
        pos.y = pos.y * 1.5 + 0.5
        pos.z = pos.z * 0.8 - 0.2

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, batch_ops)
    app.initialize()
    app.update()
    return app


def _setup_query_batch_ops(entity_count: int) -> App:
    """Setup app for Query batch operations benchmark."""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(
                Position(x=float(i), y=float(i * 2), z=float(i * 3)),
                Marker(),
            )

    def iter_ops(query: Query[Mut[Position]]) -> None:
        for pos in query:
            pos.x = pos.x * 2.0 + 1.0
            pos.y = pos.y * 1.5 + 0.5
            pos.z = pos.z * 0.8 - 0.2

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, iter_ops)
    app.initialize()
    app.update()
    return app


def _setup_view_complex_ops(entity_count: int) -> App:
    """Setup app for View complex operations benchmark."""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            fi = float(i)
            commands.spawn(Position(x=fi, y=fi * 2, z=fi * 3), Marker())

    def complex_ops(view: View[Mut[Position], Marker]) -> None:
        pos = view.column_mut(Position)
        pos.x = pos.x * 2.0 + pos.y * 0.5 - pos.z * 0.1
        pos.y = pos.y * 1.5 + pos.z * 0.3 + pos.x * 0.2
        pos.z = pos.z * 0.8 - pos.x * 0.1 + pos.y * 0.15

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, complex_ops)
    app.initialize()
    app.update()
    return app


def _setup_query_complex_ops(entity_count: int) -> App:
    """Setup app for Query complex operations benchmark."""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            fi = float(i)
            commands.spawn(Position(x=fi, y=fi * 2, z=fi * 3), Marker())

    def complex_ops(query: Query[Mut[Position]]) -> None:
        for pos in query:
            new_x = pos.x * 2.0 + pos.y * 0.5 - pos.z * 0.1
            new_y = pos.y * 1.5 + pos.z * 0.3 + pos.x * 0.2
            new_z = pos.z * 0.8 - pos.x * 0.1 + pos.y * 0.15
            pos.x = new_x
            pos.y = new_y
            pos.z = new_z

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, complex_ops)
    app.initialize()
    app.update()
    return app


ENTITY_COUNTS = [1000, 10000]


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_sum_query(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark Query iteration sum."""
    app = _setup_query_sum(entity_count)
    benchmark(app.update)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_sum_view_numba(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark View + Numba sum."""
    app = _setup_view_numba_sum(entity_count)
    benchmark(app.update)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_batch_ops_query(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark Query iteration with simple operations."""
    app = _setup_query_batch_ops(entity_count)
    benchmark(app.update)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_batch_ops_view(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark View batch operations."""
    app = _setup_view_batch_ops(entity_count)
    benchmark(app.update)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_complex_ops_query(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark Query iteration with complex operations."""
    app = _setup_query_complex_ops(entity_count)
    benchmark(app.update)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_complex_ops_view(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark View batch complex operations."""
    app = _setup_view_complex_ops(entity_count)
    benchmark(app.update)


def test_physics_query_10000(benchmark: BenchmarkFixture) -> None:
    """Benchmark Query with physics-like computation (10k entities)."""
    entity_count = 10000

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), float(i) * 0.5, 0.0))

    def physics_query(query: Query[Mut[Transform]]) -> None:
        dt = 0.016
        for t in query:
            pos = t.translation.x
            vel = t.translation.y
            new_pos = pos + vel * dt + 0.5 * vel * vel * dt
            t.translation.x = new_pos

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, physics_query)
    app.initialize()

    benchmark(app.update)


def test_physics_view_numba_10000(benchmark: BenchmarkFixture) -> None:
    """Benchmark View + Numba with physics computation (10k entities)."""
    _warmup_numba()
    entity_count = 10000

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), float(i) * 0.5, 0.0))

    def physics_view(view: View[Mut[Transform]]) -> None:
        for batch in view.iter_batches():
            pos = batch.column_mut(Transform)
            vel = batch.column_mut(Transform)
            numba_physics(pos, vel, 0.016)

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, physics_view)
    app.initialize()

    benchmark(app.update)
