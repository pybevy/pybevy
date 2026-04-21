"""Benchmarks for component storage: Wrapper vs PyObject storage.

Tests performance characteristics of:
- Wrapper storage (primitive-only components stored as bytes)
- PyObject storage (components with non-primitives stored as Python objects)
- Query iteration, spawning, mutations across storage types
"""

from dataclasses import dataclass

import pytest
from pytest_benchmark.fixture import BenchmarkFixture

from pybevy.app import App, ScheduleRunnerPlugin, Startup, Update
from pybevy.decorators import component
from pybevy.ecs import Commands, Component, Mut, Query, View


@component
@dataclass
class WrapperPosition(Component):
    x: float
    y: float
    z: float


@component
@dataclass
class WrapperVelocity(Component):
    vx: float
    vy: float
    vz: float


@component(storage="python")
@dataclass
class PyObjectPosition(Component):
    x: float
    y: float
    z: float
    data: str


@component
@dataclass
class Marker(Component):
    pass



def _setup_query_wrapper_read(entity_count: int) -> App:
    """Setup app for wrapper storage query read benchmark."""

    def spawn(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(
                WrapperPosition(x=float(i), y=float(i * 2), z=float(i * 3))
            )

    def query_read(query: Query[WrapperPosition]) -> None:
        total = 0.0
        for comp in query:
            total += comp.x + comp.y + comp.z

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, spawn)
    app.add_systems(Update, query_read)
    app.initialize()
    app.update()
    return app


def _setup_query_pyobject_read(entity_count: int) -> App:
    """Setup app for PyObject storage query read benchmark."""

    def spawn(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(
                PyObjectPosition(
                    x=float(i), y=float(i * 2), z=float(i * 3), data="test"
                )
            )

    def query_read(query: Query[PyObjectPosition]) -> None:
        total = 0.0
        for comp in query:
            total += comp.x + comp.y + comp.z

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, spawn)
    app.add_systems(Update, query_read)
    app.initialize()
    app.update()
    return app


def _setup_query_wrapper_write(entity_count: int) -> App:
    """Setup app for wrapper storage query write benchmark."""

    def spawn(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(
                WrapperPosition(x=float(i), y=float(i * 2), z=float(i * 3)), Marker()
            )

    def query_write(query: Query[Mut[WrapperPosition]]) -> None:
        for pos in query:
            pos.x = pos.x * 2.0 + 1.0
            pos.y = pos.y * 1.5 + 0.5
            pos.z = pos.z * 0.8 - 0.2

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, spawn)
    app.add_systems(Update, query_write)
    app.initialize()
    app.update()
    return app


def _setup_query_pyobject_write(entity_count: int) -> App:
    """Setup app for PyObject storage query write benchmark."""

    def spawn(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(
                PyObjectPosition(
                    x=float(i), y=float(i * 2), z=float(i * 3), data="test"
                ),
                Marker(),
            )

    def query_write(query: Query[Mut[PyObjectPosition]]) -> None:
        for pos in query:
            pos.x = pos.x * 2.0 + 1.0
            pos.y = pos.y * 1.5 + 0.5
            pos.z = pos.z * 0.8 - 0.2

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, spawn)
    app.add_systems(Update, query_write)
    app.initialize()
    app.update()
    return app


def _setup_view_wrapper_write(entity_count: int) -> App:
    """Setup app for wrapper storage View write benchmark."""

    def spawn(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(
                WrapperPosition(x=float(i), y=float(i * 2), z=float(i * 3)), Marker()
            )

    def view_write(view: View[Mut[WrapperPosition], Marker]) -> None:
        pos = view.column_mut(WrapperPosition)
        pos.x = pos.x * 2.0 + 1.0
        pos.y = pos.y * 1.5 + 0.5
        pos.z = pos.z * 0.8 - 0.2

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, spawn)
    app.add_systems(Update, view_write)
    app.initialize()
    app.update()
    return app


ENTITY_COUNTS = [1000, 10000]


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_read_query_wrapper(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark Query read with wrapper storage."""
    app = _setup_query_wrapper_read(entity_count)
    benchmark(app.update)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_read_query_pyobject(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark Query read with PyObject storage."""
    app = _setup_query_pyobject_read(entity_count)
    benchmark(app.update)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_write_query_wrapper(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark Query write with wrapper storage."""
    app = _setup_query_wrapper_write(entity_count)
    benchmark(app.update)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_write_query_pyobject(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark Query write with PyObject storage."""
    app = _setup_query_pyobject_write(entity_count)
    benchmark(app.update)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_write_view_wrapper(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark View write with wrapper storage."""
    app = _setup_view_wrapper_write(entity_count)
    benchmark(app.update)


def test_multi_component_wrapper_10000(benchmark: BenchmarkFixture) -> None:
    """Benchmark multi-component query with wrapper storage (10k entities)."""
    entity_count = 10000

    def spawn(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(
                WrapperPosition(x=float(i), y=float(i * 2), z=float(i * 3)),
                WrapperVelocity(vx=1.0, vy=2.0, vz=3.0),
            )

    def query_multi(query: Query[tuple[Mut[WrapperPosition], WrapperVelocity]]) -> None:
        for pos, vel in query:
            pos.x += vel.vx
            pos.y += vel.vy
            pos.z += vel.vz

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, spawn)
    app.add_systems(Update, query_multi)
    app.initialize()

    benchmark(app.update)
