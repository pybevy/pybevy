#!/usr/bin/env python3
"""Paper Table 2: Query vs View API vs Numba scaling benchmark.

Measures per-entity throughput at increasing entity counts across all three
execution tiers with proper statistical treatment (median +/- IQR across
multiple rounds).

Usage:
    poetry run python benches/paper/scaling.py
    poetry run python benches/paper/scaling.py --counts 1000 10000
    poetry run python benches/paper/scaling.py --rounds 1 --iterations 3  # quick smoke test

Requirements:
    - PyBevy built in release mode: poetry run maturin develop --release
    - For Numba benchmarks: poetry install -E numba
"""

from __future__ import annotations

import argparse
import time

from benches.paper.bench_utils import (
    BenchConfig,
    BenchResult,
    add_bench_args,
    print_system_info,
)
from pybevy.app import (
    App,
    RunMode,
    ScheduleRunnerPlugin,
    Startup,
    Update,
)
from pybevy.ecs import Commands, Mut, Query, View
from pybevy.transform import Transform

DEFAULT_COUNTS = [1_000, 5_000, 10_000, 100_000, 1_000_000, 10_000_000]


# =============================================================================
# Benchmark Helpers
# =============================================================================


def _make_query_app(entity_count: int) -> App:
    """Create an app with Query-based increment system."""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

    def increment(query: Query[Mut[Transform]]) -> None:
        for transform in query:
            transform.translation.x += 1.0

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, increment)
    app.initialize()
    return app


def _make_view_app(entity_count: int) -> App:
    """Create an app with View API (bytecode VM) increment system."""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

    def increment(view: View[Mut[Transform]]) -> None:
        col = view.column_mut(Transform)
        col.translation.x += 1.0

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, increment)
    app.initialize()
    return app


def _make_numba_app(entity_count: int) -> App:
    """Create an app with ViewColumn+Numba increment system."""
    import numba  # type: ignore[import-untyped]

    @numba.jit(nopython=True)
    def increment_column(col: numba.float32[:]) -> None:  # type: ignore[name-defined]
        for i in range(len(col)):
            col[i] = col[i] + 1.0

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

    def increment(view: View[Mut[Transform]]) -> None:
        for batch in view.iter_batches():
            col = batch.column_mut(Transform)
            increment_column(col.translation.x)

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, increment)
    app.initialize()
    return app


def bench_query_scaling(
    entity_count: int, cfg: BenchConfig
) -> BenchResult:
    """Benchmark Query iteration at given entity count."""
    app = _make_query_app(entity_count)
    # Warmup
    for _ in range(cfg.warmup):
        app.update()

    round_medians: list[float] = []
    for _ in range(cfg.rounds):
        times: list[float] = []
        for _ in range(cfg.iterations):
            t0 = time.perf_counter()
            app.update()
            times.append((time.perf_counter() - t0) * 1000)
        times.sort()
        round_medians.append(times[len(times) // 2])

    return _medians_to_result(round_medians)


def bench_view_scaling(
    entity_count: int, cfg: BenchConfig
) -> BenchResult:
    """Benchmark View API (bytecode VM) at given entity count."""
    app = _make_view_app(entity_count)
    # Warmup
    for _ in range(cfg.warmup):
        app.update()

    round_medians: list[float] = []
    for _ in range(cfg.rounds):
        times: list[float] = []
        for _ in range(cfg.iterations):
            t0 = time.perf_counter()
            app.update()
            times.append((time.perf_counter() - t0) * 1000)
        times.sort()
        round_medians.append(times[len(times) // 2])

    return _medians_to_result(round_medians)


def bench_numba_scaling(
    entity_count: int, cfg: BenchConfig
) -> BenchResult:
    """Benchmark ViewColumn+Numba at given entity count."""
    app = _make_numba_app(entity_count)
    # Warmup (includes Numba JIT compilation)
    for _ in range(cfg.warmup):
        app.update()

    round_medians: list[float] = []
    for _ in range(cfg.rounds):
        times: list[float] = []
        for _ in range(cfg.iterations):
            t0 = time.perf_counter()
            app.update()
            times.append((time.perf_counter() - t0) * 1000)
        times.sort()
        round_medians.append(times[len(times) // 2])

    return _medians_to_result(round_medians)


def _medians_to_result(round_medians: list[float]) -> BenchResult:
    """Convert round medians to BenchResult."""
    import statistics

    round_medians.sort()
    n = len(round_medians)
    q1_idx = n // 4
    q3_idx = (3 * n) // 4
    return BenchResult(
        median_ms=statistics.median(round_medians),
        q1_ms=round_medians[q1_idx],
        q3_ms=round_medians[q3_idx],
        min_ms=round_medians[0],
        max_ms=round_medians[-1],
        std_ms=statistics.stdev(round_medians) if n > 1 else 0.0,
        round_medians=round_medians,
    )


# =============================================================================
# Main
# =============================================================================


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Paper Table 2: Query vs ViewColumn+Numba scaling"
    )
    add_bench_args(parser)
    parser.add_argument(
        "--counts",
        type=int,
        nargs="+",
        default=DEFAULT_COUNTS,
        help="Entity counts to benchmark (default: 1K 5K 10K 100K 1M 10M)",
    )
    parser.add_argument(
        "--skip-query-above",
        type=int,
        default=100_000,
        help="Skip Query benchmark above this entity count (default: 100000)",
    )
    args = parser.parse_args()
    cfg = BenchConfig.from_args(args)

    print_system_info()

    # Check Numba
    try:
        import numba  # noqa: F401

        has_numba = True
    except ImportError:
        has_numba = False
        print("WARNING: Numba not installed. Numba columns will be skipped.")
        print("Install with: poetry install -E numba\n")

    # Warmup View bytecode VM
    print("Warming up View bytecode VM...", flush=True)
    warmup_view = _make_view_app(10)
    warmup_view.update()

    # Warmup Numba JIT with tiny app
    if has_numba:
        print("Warming up Numba JIT...", flush=True)
        warmup_app = _make_numba_app(10)
        warmup_app.update()

    print()

    # Print header
    w = 22  # column width
    print("=" * 100)
    title = f"Scaling: Query vs View vs Numba ({cfg.warmup} warmup, {cfg.iterations} iter, {cfg.rounds} rounds)"
    print(title)
    print("=" * 100)
    hdr = f"{'Entities':>12}  {'Query':>{w}}  {'View API':>{w}}"
    if has_numba:
        hdr += f"  {'Numba':>{w}}"
    print(hdr)
    print("-" * 100)

    for count in sorted(args.counts):
        row = f"{count:>12,}"

        # Query
        if count <= args.skip_query_above:
            print(f"  Benchmarking Query @ {count:,}...", end="", flush=True)
            query_result = bench_query_scaling(count, cfg)
            print(f" {query_result.median_ms:.3f}ms")
            row += f"  {query_result.format_compact():>{w}}"
        else:
            row += f"  {'(skipped)':>{w}}"
            query_result = None

        # View API (bytecode VM)
        print(f"  Benchmarking View @ {count:,}...", end="", flush=True)
        view_result = bench_view_scaling(count, cfg)
        print(f" {view_result.median_ms:.3f}ms")
        row += f"  {view_result.format_compact():>{w}}"

        # Numba
        if has_numba:
            print(f"  Benchmarking Numba @ {count:,}...", end="", flush=True)
            numba_result = bench_numba_scaling(count, cfg)
            print(f" {numba_result.median_ms:.3f}ms")
            row += f"  {numba_result.format_compact():>{w}}"

        print(row)

    print("=" * 100)
    print()
    print("Note: Query skipped above", f"{args.skip_query_above:,}", "entities (too slow).")
    print()


if __name__ == "__main__":
    main()
