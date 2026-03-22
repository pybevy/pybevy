"""Benchmarks for Commands operations: batch spawning."""

import numpy as np
from pytest_benchmark.fixture import BenchmarkFixture

from pybevy.app import App, RunMode, ScheduleRunnerPlugin, Startup
from pybevy.decorators import component
from pybevy.ecs import Commands, Component, World
from pybevy.transform import Transform


@component
class Marker(Component):
    """Marker component for benchmarking."""


def _bench_batch_spawn(entity_count: int) -> None:
    """Helper to benchmark batch spawn."""
    positions = np.random.rand(entity_count, 3).astype(np.float32) * 100

    def setup_batch(world: World) -> None:
        batch = Transform.from_numpy(translation=positions)
        world.commands().spawn_batch(batch, Marker())

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup_batch)
    app.run()


def _bench_normal_spawn(entity_count: int) -> None:
    """Helper to benchmark normal spawn."""
    positions = np.random.rand(entity_count, 3).astype(np.float32) * 100

    def setup_normal(commands: Commands) -> None:
        for i in range(entity_count):
            x, y, z = positions[i]
            commands.spawn(Transform.from_xyz(x, y, z), Marker())

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup_normal)
    app.run()


def _bench_batch_scaling(entity_count: int) -> None:
    """Helper to benchmark batch spawn with full transform data."""
    positions = np.random.rand(entity_count, 3).astype(np.float32) * 100
    rotations = np.zeros((entity_count, 4), dtype=np.float32)
    rotations[:, 3] = 1.0

    def setup(world: World) -> None:
        batch = Transform.from_numpy(translation=positions, rotation=rotations)
        world.commands().spawn_batch(batch, Marker())

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.run()


def test_spawn_100_batch(benchmark: BenchmarkFixture) -> None:
    """Benchmark batch spawn with 100 entities."""
    benchmark(_bench_batch_spawn, 100)


def test_spawn_100_normal(benchmark: BenchmarkFixture) -> None:
    """Benchmark normal spawn with 100 entities."""
    benchmark(_bench_normal_spawn, 100)


def test_spawn_1000_batch(benchmark: BenchmarkFixture) -> None:
    """Benchmark batch spawn with 1000 entities."""
    benchmark(_bench_batch_spawn, 1000)


def test_spawn_1000_normal(benchmark: BenchmarkFixture) -> None:
    """Benchmark normal spawn with 1000 entities."""
    benchmark(_bench_normal_spawn, 1000)


def test_spawn_10000_batch(benchmark: BenchmarkFixture) -> None:
    """Benchmark batch spawn with 10000 entities."""
    benchmark(_bench_batch_spawn, 10000)


def test_spawn_10000_normal(benchmark: BenchmarkFixture) -> None:
    """Benchmark normal spawn with 10000 entities."""
    benchmark(_bench_normal_spawn, 10000)


def test_batch_scaling_100(benchmark: BenchmarkFixture) -> None:
    """Benchmark batch spawn with full transform data (100 entities)."""
    benchmark(_bench_batch_scaling, 100)


def test_batch_scaling_1000(benchmark: BenchmarkFixture) -> None:
    """Benchmark batch spawn with full transform data (1000 entities)."""
    benchmark(_bench_batch_scaling, 1000)


def test_batch_scaling_10000(benchmark: BenchmarkFixture) -> None:
    """Benchmark batch spawn with full transform data (10000 entities)."""
    benchmark(_bench_batch_scaling, 10000)


def test_batch_scaling_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark batch spawn with full transform data (100000 entities)."""
    benchmark(_bench_batch_scaling, 100000)


def test_batch_scaling_1000000(benchmark: BenchmarkFixture) -> None:
    """Benchmark batch spawn with full transform data (1000000 entities)."""
    benchmark(_bench_batch_scaling, 1000000)


def test_batch_spawn_with_full_transform_data(benchmark: BenchmarkFixture) -> None:
    """Benchmark batch spawning with positions, rotations, and scales."""
    count = 100_000
    positions = np.random.rand(count, 3).astype(np.float32) * 100
    rotations = np.random.rand(count, 4).astype(np.float32)
    # Normalize quaternions
    norms = np.sqrt(np.sum(rotations**2, axis=1, keepdims=True))
    rotations = rotations / norms
    scales = np.ones((count, 3), dtype=np.float32)

    def setup(world: World) -> None:
        batch = Transform.from_numpy(
            translation=positions, rotation=rotations, scale=scales
        )
        entities = world.commands().spawn_batch(batch, Marker())
        assert len(entities) == count

    def bench():
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
        app.add_systems(Startup, setup)
        app.run()

    benchmark(bench)


def test_batch_spawn_positions_only(benchmark: BenchmarkFixture) -> None:
    """Benchmark batch spawning with positions only (fastest)."""
    count = 100_000
    positions = np.random.rand(count, 3).astype(np.float32) * 100

    def setup(world: World) -> None:
        batch = Transform.from_numpy(translation=positions)
        world.commands().spawn_batch(batch, Marker())

    def bench():
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
        app.add_systems(Startup, setup)
        app.run()

    benchmark(bench)
