"""Benchmarks for batch spawning: isolating spawn cost, dtype comparisons."""

from collections.abc import Callable

import numpy as np
from pytest_benchmark.fixture import BenchmarkFixture

from pybevy.app import App, Startup
from pybevy.camera import Visibility
from pybevy.decorators import component
from pybevy.ecs import Commands, Component, World
from pybevy.transform import Transform


@component
class Marker(Component):
    pass


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _run_batch(setup_fn: Callable[..., None]) -> None:
    app = App()
    app.add_systems(Startup, setup_fn)
    app.initialize()
    app.update()


def _spawn_batch_positions(entity_count: int) -> None:
    positions = np.random.rand(entity_count, 3).astype(np.float32) * 100

    def setup(world: World) -> None:
        batch = Transform.from_numpy(translation=positions)
        world.commands().spawn_batch(batch, Marker())

    _run_batch(setup)


def _spawn_normal(entity_count: int) -> None:
    positions = np.random.rand(entity_count, 3).astype(np.float32) * 100

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            x, y, z = float(positions[i, 0]), float(positions[i, 1]), float(positions[i, 2])
            commands.spawn(Transform.from_xyz(x, y, z), Marker())

    _run_batch(setup)


def _spawn_batch_full(entity_count: int) -> None:
    positions = np.random.rand(entity_count, 3).astype(np.float32) * 100
    rotations = np.zeros((entity_count, 4), dtype=np.float32)
    rotations[:, 3] = 1.0
    scales = np.ones((entity_count, 3), dtype=np.float32)

    def setup(world: World) -> None:
        batch = Transform.from_numpy(
            translation=positions, rotation=rotations, scale=scales
        )
        world.commands().spawn_batch(batch)

    _run_batch(setup)


def _spawn_batch_with_visibility(entity_count: int) -> None:
    positions = np.random.rand(entity_count, 3).astype(np.float32) * 100
    visibility = np.random.choice([True, False], size=entity_count)

    def setup(world: World) -> None:
        t_batch = Transform.from_numpy(translation=positions)
        v_batch = Visibility.from_numpy(visibility)
        world.commands().spawn_batch(t_batch, v_batch, Marker())

    _run_batch(setup)


def _spawn_batch_with_uniforms(entity_count: int) -> None:
    positions = np.random.rand(entity_count, 3).astype(np.float32) * 100

    def setup(world: World) -> None:
        t_batch = Transform.from_numpy(translation=positions)
        world.commands().spawn_batch(
            t_batch,
            Visibility.VISIBLE,
            Marker(),
        )

    _run_batch(setup)


# ---------------------------------------------------------------------------
# Batch vs normal spawn comparison
# ---------------------------------------------------------------------------


def test_batch_vs_normal_100_batch(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_positions, 100)


def test_batch_vs_normal_100_normal(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_normal, 100)


def test_batch_vs_normal_1000_batch(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_positions, 1000)


def test_batch_vs_normal_1000_normal(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_normal, 1000)


def test_batch_vs_normal_10000_batch(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_positions, 10000)


def test_batch_vs_normal_10000_normal(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_normal, 10000)


# ---------------------------------------------------------------------------
# Positions-only vs full transform (measures per-array overhead)
# ---------------------------------------------------------------------------


def test_positions_only_10000(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_positions, 10000)


def test_full_transform_10000(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_full, 10000)


def test_positions_only_100000(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_positions, 100000)


def test_full_transform_100000(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_full, 100000)


# ---------------------------------------------------------------------------
# Visibility batch benchmarks
# ---------------------------------------------------------------------------


def test_with_visibility_10000(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_with_visibility, 10000)


def test_transform_only_10000(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_positions, 10000)


def test_with_visibility_100000(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_with_visibility, 100000)


def test_transform_only_100000(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_positions, 100000)


# ---------------------------------------------------------------------------
# Uniform component overhead
# ---------------------------------------------------------------------------


def test_with_uniforms_10000(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_with_uniforms, 10000)


def test_with_uniforms_100000(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_with_uniforms, 100000)


# ---------------------------------------------------------------------------
# World vs deferred Commands batch spawn
# ---------------------------------------------------------------------------


def _spawn_batch_commands(entity_count: int) -> None:
    """Batch spawn via deferred Commands (queued, applied at flush)."""
    positions = np.random.rand(entity_count, 3).astype(np.float32) * 100

    def setup(commands: Commands) -> None:
        batch = Transform.from_numpy(translation=positions)
        commands.spawn_batch(batch, Marker())

    _run_batch(setup)


def test_batch_world_vs_commands_1000_world(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_positions, 1000)


def test_batch_world_vs_commands_1000_commands(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_commands, 1000)


def test_batch_world_vs_commands_10000_world(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_positions, 10000)


def test_batch_world_vs_commands_10000_commands(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_commands, 10000)


def test_batch_world_vs_commands_100000_world(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_positions, 100000)


def test_batch_world_vs_commands_100000_commands(benchmark: BenchmarkFixture) -> None:
    benchmark(_spawn_batch_commands, 100000)
