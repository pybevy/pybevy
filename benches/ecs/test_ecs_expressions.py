"""Benchmarks for View expression operations: reductions, conditionals, comparisons.

This file tests the bytecode VM expression system including:
- Reduction operations (sum, mean, max, count)
- Comparison operators (==, !=, <, <=, >, >=)
- Conditional logic (where, nested where)
- Logical operators (AND, OR, NOT)
- Combined operations
"""

import pytest
from pytest_benchmark.fixture import BenchmarkFixture

from pybevy.app import RunMode, ScheduleRunnerPlugin
from pybevy.decorators import component
from pybevy.ecs import Commands, Component, Query, View
from pybevy.expr import where
from pybevy.prelude import *
from pybevy.transform import Transform


@component
class Marker(Component):
    """Marker component for benchmarking."""


def _spawn_entities(commands: Commands, count: int) -> None:
    """Spawn entities with sequential values in Transform.translation.x."""
    for i in range(count):
        commands.spawn(Marker(), Transform.from_xyz(float(i), float(i * 2), 0.0))


def _bench_reduce_view(
    operation: str, entity_count: int, verify_fn=None
) -> tuple[float, App]:
    """Helper to benchmark View reduction operations."""
    result = {}

    def setup(commands: Commands) -> None:
        _spawn_entities(commands, entity_count)

    def reduce_system(view: View[Transform, With[Marker]]) -> None:
        transform = view.column(Transform)
        x = transform.translation.x
        if operation == "sum":
            result["value"] = view.reduce_sum(x)
        elif operation == "mean":
            result["value"] = view.reduce_mean(x)
        elif operation == "max":
            result["value"] = view.reduce_max(x)
        elif operation == "count":
            result["value"] = view.reduce_count()

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, reduce_system)
    app.run()

    value = result["value"]
    if verify_fn:
        verify_fn(value, entity_count)

    return value, app


def _bench_reduce_query(
    operation: str, entity_count: int, verify_fn=None
) -> tuple[float, App]:
    """Helper to benchmark Query reduction operations."""
    result = {}

    def setup(commands: Commands) -> None:
        _spawn_entities(commands, entity_count)

    def query_system(query: Query[tuple[Transform, Marker]]) -> None:
        if operation == "sum":
            total = 0.0
            for transform, _ in query:
                total += transform.translation.x
            result["value"] = total
        elif operation == "mean":
            total = 0.0
            count = 0
            for transform, _ in query:
                total += transform.translation.x
                count += 1
            result["value"] = total / count if count > 0 else 0.0
        elif operation == "max":
            maximum = float("-inf")
            for transform, _ in query:
                maximum = max(maximum, transform.translation.x)
            result["value"] = maximum
        elif operation == "count":
            count = 0
            for _ in query:
                count += 1
            result["value"] = count

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, query_system)
    app.run()

    value = result["value"]
    if verify_fn:
        verify_fn(value, entity_count)

    return value, app


def _bench_comparison_operator(operator_fn, setup_fn) -> App:
    """Helper to benchmark comparison operators."""
    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup_fn)
    app.add_systems(Update, operator_fn)
    app.initialize()
    app.update()
    return app


ENTITY_COUNTS = [100, 1000, 10000]


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_reduce_sum_view(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark reduce_sum using View API."""

    def verify(value, count):
        expected = sum(range(count))
        assert abs(value - expected) / expected < 0.001

    def bench():
        return _bench_reduce_view("sum", entity_count, verify)[0]

    benchmark(bench)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_reduce_sum_query(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark sum using traditional Query iteration."""

    def verify(value, count):
        expected = sum(range(count))
        assert abs(value - expected) / expected < 0.001

    def bench():
        return _bench_reduce_query("sum", entity_count, verify)[0]

    benchmark(bench)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_reduce_mean_view(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark reduce_mean using View API."""

    def verify(value, count):
        expected = sum(range(count)) / count
        assert abs(value - expected) / expected < 0.001

    def bench():
        return _bench_reduce_view("mean", entity_count, verify)[0]

    benchmark(bench)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_reduce_mean_query(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark mean using traditional Query iteration."""

    def verify(value, count):
        expected = sum(range(count)) / count
        assert abs(value - expected) / expected < 0.001

    def bench():
        return _bench_reduce_query("mean", entity_count, verify)[0]

    benchmark(bench)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_reduce_max_view(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark reduce_max using View API."""

    def verify(value, count):
        expected = float(count - 1)
        assert abs(value - expected) / expected < 0.001

    def bench():
        return _bench_reduce_view("max", entity_count, verify)[0]

    benchmark(bench)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_reduce_max_query(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark max using traditional Query iteration."""

    def verify(value, count):
        expected = float(count - 1)
        assert abs(value - expected) / expected < 0.001

    def bench():
        return _bench_reduce_query("max", entity_count, verify)[0]

    benchmark(bench)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_reduce_count_view(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark reduce_count using View API."""

    def verify(value, count):
        assert value == count

    def bench():
        return _bench_reduce_view("count", entity_count, verify)[0]

    benchmark(bench)


@pytest.mark.parametrize("entity_count", ENTITY_COUNTS)
def test_reduce_count_query(benchmark: BenchmarkFixture, entity_count: int) -> None:
    """Benchmark count using traditional Query iteration."""

    def verify(value, count):
        assert value == count

    def bench():
        return _bench_reduce_query("count", entity_count, verify)[0]

    benchmark(bench)


NUM_ENTITIES = 100_000


def test_compare_equal_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark == comparison operator with 100k entities."""

    def setup(commands: Commands) -> None:
        for i in range(NUM_ENTITIES):
            x = float(i % 100)
            commands.spawn(Transform.from_xyz(x, x if i % 2 == 0 else x + 1, 0.0))

    def compare_equal(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        x = transform.translation.x
        y = transform.translation.y
        transform.translation.z = where(x == y, 1.0, 0.0)

    app = _bench_comparison_operator(compare_equal, setup)
    benchmark(app.update)


def test_compare_not_equal_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark != comparison operator with 100k entities."""

    def setup(commands: Commands) -> None:
        for i in range(NUM_ENTITIES):
            x = float(i % 100)
            commands.spawn(Transform.from_xyz(x, x + 1, 0.0))

    def compare_not_equal(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        x = transform.translation.x
        y = transform.translation.y
        transform.translation.z = where(x != y, 1.0, 0.0)

    app = _bench_comparison_operator(compare_not_equal, setup)
    benchmark(app.update)


def test_compare_less_than_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark < comparison operator with 100k entities."""

    def setup(commands: Commands) -> None:
        for i in range(NUM_ENTITIES):
            x = float(i % 100)
            commands.spawn(Transform.from_xyz(x, x + 10, 0.0))

    def compare_less_than(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        x = transform.translation.x
        y = transform.translation.y
        transform.translation.z = where(x < y, 1.0, 0.0)

    app = _bench_comparison_operator(compare_less_than, setup)
    benchmark(app.update)


def test_compare_greater_than_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark > comparison operator with 100k entities."""

    def setup(commands: Commands) -> None:
        for i in range(NUM_ENTITIES):
            x = float(i % 100)
            commands.spawn(Transform.from_xyz(x, x - 10, 0.0))

    def compare_greater_than(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        x = transform.translation.x
        y = transform.translation.y
        transform.translation.z = where(x > y, 1.0, 0.0)

    app = _bench_comparison_operator(compare_greater_than, setup)
    benchmark(app.update)


def test_where_constants_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark where() with constant true/false values (100k entities)."""

    def setup(commands: Commands) -> None:
        for i in range(NUM_ENTITIES):
            x = float(i % 200 - 100)
            commands.spawn(Transform.from_xyz(x, 0.0, 0.0))

    def where_constants(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        x = transform.translation.x
        transform.translation.y = where(x > 0.0, 10.0, -10.0)

    app = _bench_comparison_operator(where_constants, setup)
    benchmark(app.update)


def test_where_expressions_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark where() with expression values (100k entities)."""

    def setup(commands: Commands) -> None:
        for i in range(NUM_ENTITIES):
            x = float(i % 200 - 100)
            commands.spawn(Transform.from_xyz(x, 0.0, 0.0))

    def where_expressions(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        x = transform.translation.x
        transform.translation.y = where(x > 0.0, x * 2.0, x * -1.0)

    app = _bench_comparison_operator(where_expressions, setup)
    benchmark(app.update)


def test_where_nested_2_levels_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark nested where() - 2 levels (3-way branch, 100k entities)."""

    def setup(commands: Commands) -> None:
        for i in range(NUM_ENTITIES):
            x = float(i % 300 - 150)
            commands.spawn(Transform.from_xyz(x, 0.0, 0.0))

    def nested_where_2(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        x = transform.translation.x
        transform.translation.y = where(x < 0.0, 1.0, where(x == 0.0, 2.0, 3.0))

    app = _bench_comparison_operator(nested_where_2, setup)
    benchmark(app.update)


def test_where_nested_3_levels_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark nested where() - 3 levels (4-way branch, 100k entities)."""

    def setup(commands: Commands) -> None:
        for i in range(NUM_ENTITIES):
            x = float(i % 400 - 200)
            commands.spawn(Transform.from_xyz(x, 0.0, 0.0))

    def nested_where_3(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        x = transform.translation.x
        transform.translation.y = where(
            x < -50.0,
            1.0,
            where(x < 0.0, 2.0, where(x < 50.0, 3.0, 4.0)),
        )

    app = _bench_comparison_operator(nested_where_3, setup)
    benchmark(app.update)


def test_logical_and_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark AND operator (&) for combining conditions (100k entities)."""
    import random

    def setup(commands: Commands) -> None:
        random.seed(42)
        for _ in range(NUM_ENTITIES):
            health = random.uniform(0.0, 100.0)
            shield = random.uniform(0.0, 100.0)
            commands.spawn(Transform.from_xyz(health, shield, 0.0))

    def and_operator(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        health = transform.translation.x
        shield = transform.translation.y
        is_vulnerable = (health < 30.0) & (shield <= 10.0)
        transform.scale.x = where(is_vulnerable, 2.0, 1.0)

    app = _bench_comparison_operator(and_operator, setup)
    benchmark(app.update)


def test_logical_or_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark OR operator (|) for alternative conditions (100k entities)."""
    import random

    def setup(commands: Commands) -> None:
        random.seed(42)
        for _ in range(NUM_ENTITIES):
            health = random.uniform(0.0, 100.0)
            shield = random.uniform(0.0, 100.0)
            commands.spawn(Transform.from_xyz(health, shield, 0.0))

    def or_operator(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        health = transform.translation.x
        shield = transform.translation.y
        needs_healing = (health < 50.0) | (shield < 30.0)
        transform.scale.y = where(needs_healing, 3.0, 1.0)

    app = _bench_comparison_operator(or_operator, setup)
    benchmark(app.update)


def test_logical_not_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark NOT operator (~) for negation (100k entities)."""
    import random

    def setup(commands: Commands) -> None:
        random.seed(42)
        for _ in range(NUM_ENTITIES):
            health = random.uniform(0.0, 100.0)
            commands.spawn(Transform.from_xyz(health, 0.0, 0.0))

    def not_operator(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        health = transform.translation.x
        is_critical = health < 20.0
        is_healthy = ~is_critical
        transform.scale.z = where(is_healthy, 1.5, 1.0)

    app = _bench_comparison_operator(not_operator, setup)
    benchmark(app.update)


def test_logical_complex_and_or_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark complex (A AND B) OR C logic (100k entities)."""
    import random

    def setup(commands: Commands) -> None:
        random.seed(42)
        for _ in range(NUM_ENTITIES):
            health = random.uniform(0.0, 100.0)
            shield = random.uniform(0.0, 100.0)
            commands.spawn(Transform.from_xyz(health, shield, 0.0))

    def complex_and_or(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        health = transform.translation.x
        shield = transform.translation.y
        low_health_no_shield = (health < 30.0) & (shield <= 10.0)
        critical = health < 15.0
        is_danger = low_health_no_shield | critical
        transform.translation.z = where(is_danger, 999.0, 0.0)

    app = _bench_comparison_operator(complex_and_or, setup)
    benchmark(app.update)


def test_logical_multiple_and_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark multiple AND operators chained (100k entities)."""
    import random

    def setup(commands: Commands) -> None:
        random.seed(42)
        for _ in range(NUM_ENTITIES):
            health = random.uniform(0.0, 100.0)
            shield = random.uniform(0.0, 100.0)
            commands.spawn(Transform.from_xyz(health, shield, 0.0))

    def multiple_and(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        health = transform.translation.x
        shield = transform.translation.y
        condition = (health > 80.0) & (shield > 60.0) & (health < 100.0)
        transform.rotation.x = where(condition, 1.0, 0.0)

    app = _bench_comparison_operator(multiple_and, setup)
    benchmark(app.update)


def test_logical_complex_nested_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark complex nested boolean logic (100k entities)."""
    import random

    def setup(commands: Commands) -> None:
        random.seed(42)
        for _ in range(NUM_ENTITIES):
            health = random.uniform(0.0, 100.0)
            shield = random.uniform(0.0, 100.0)
            commands.spawn(Transform.from_xyz(health, shield, 0.0))

    def complex_nested(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        health = transform.translation.x
        shield = transform.translation.y
        good_both = (health > 60.0) & (shield > 30.0)
        excellent_health = (health > 80.0) & (shield > 0.0)
        combat_ready = good_both | excellent_health
        transform.rotation.z = where(combat_ready, 1.0, 0.0)

    app = _bench_comparison_operator(complex_nested, setup)
    benchmark(app.update)


def test_conditional_view_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark View with where() conditional (100k entities)."""

    def setup(commands: Commands) -> None:
        for i in range(NUM_ENTITIES):
            x = float(i % 200 - 100)
            commands.spawn(Transform.from_xyz(x, 0.0, 0.0))

    def view_conditional(view: View[Transform]) -> None:
        transform = view.column_mut(Transform)
        x = transform.translation.x
        transform.translation.y = where(x > 0.0, x * 2.0, x * -1.0)

    app = _bench_comparison_operator(view_conditional, setup)
    benchmark(app.update)


def test_conditional_query_100000(benchmark: BenchmarkFixture) -> None:
    """Benchmark Query with Python if/else (100k entities)."""

    def setup(commands: Commands) -> None:
        for i in range(NUM_ENTITIES):
            x = float(i % 200 - 100)
            commands.spawn(Transform.from_xyz(x, 0.0, 0.0))

    def query_conditional(query: Query[Transform]) -> None:
        for transform in query:
            x = transform.translation.x
            if x > 0.0:
                transform.translation.y = x * 2.0
            else:
                transform.translation.y = x * -1.0

    app = App()
    app.add_plugins(ScheduleRunnerPlugin.run_once())
    app.add_systems(Startup, setup)
    app.add_systems(Update, query_conditional)
    app.initialize()
    app.update()

    benchmark(app.update)
