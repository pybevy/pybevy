#!/usr/bin/env python3
"""Paper Table 5: Dynamic dispatch overhead benchmark.

Measures View API (bytecode VM) execution time across three workload types
at 1M entities. The static vs dynamic bridge dispatch comparison is measured
in Rust via `cargo bench --bench query_dispatch`.

This script measures the Python-observable View API execution times for:
  - Increment: single field += 1.0
  - Complex math: trigonometric expression
  - Multi-field: operations across translation.x/y/z

Usage:
    poetry run python benches/paper/dispatch_overhead.py
    poetry run python benches/paper/dispatch_overhead.py --entities 100000
    poetry run python benches/paper/dispatch_overhead.py --rounds 1 --iterations 3  # quick test

Requirements:
    - PyBevy built in release mode: poetry run maturin develop --release

Related Rust benchmark (static vs dynamic dispatch):
    cargo bench --bench query_dispatch
"""

from __future__ import annotations

import argparse
import statistics
import time

from benches.paper.bench_utils import (
    BenchConfig,
    BenchResult,
    add_bench_args,
    print_system_info,
)
from pybevy.app import App, RunMode, ScheduleRunnerPlugin, Startup, Update
from pybevy.ecs import Commands, Mut, View
from pybevy.transform import Transform

DEFAULT_ENTITIES = 1_000_000


# =============================================================================
# App Factories (3 workload types)
# =============================================================================


def _make_increment_app(entity_count: int) -> App:
    """Single field increment: translation.x += 1.0"""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

    def workload(view: View[Mut[Transform]]) -> None:
        col = view.column_mut(Transform)
        col.translation.x += 1.0

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, workload)
    app.initialize()
    return app


def _make_complex_math_app(entity_count: int) -> App:
    """Complex math: translation.x = sin(translation.x) * 2.0 + 1.0"""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i) * 0.01, 0.0, 0.0))

    def workload(view: View[Mut[Transform]]) -> None:
        col = view.column_mut(Transform)
        col.translation.x = col.translation.x.sin() * 2.0 + 1.0

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, workload)
    app.initialize()
    return app


def _make_multi_field_app(entity_count: int) -> App:
    """Multi-field: operations on translation.x, .y, .z"""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), float(i) * 0.5, 0.0))

    def workload(view: View[Mut[Transform]]) -> None:
        col = view.column_mut(Transform)
        col.translation.x += 1.0
        col.translation.y += 1.0
        col.translation.z += 1.0

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, workload)
    app.initialize()
    return app


# =============================================================================
# Benchmark Runner
# =============================================================================


def _bench_app(app: App, cfg: BenchConfig) -> BenchResult:
    """Run benchmark on a pre-initialized app."""
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


WORKLOADS = [
    ("Increment", "translation.x += 1.0", _make_increment_app),
    ("Complex Math", "sin(x) * 2.0 + 1.0", _make_complex_math_app),
    ("Multi-Field", "x += 1, y += 1, z += 1", _make_multi_field_app),
]


def main() -> None:
    """Run dispatch overhead benchmark."""
    parser = argparse.ArgumentParser(
        description="Paper Table 5: Dynamic dispatch overhead benchmark"
    )
    add_bench_args(parser)
    parser.add_argument(
        "--entities",
        type=int,
        default=DEFAULT_ENTITIES,
        help=f"Entity count (default: {DEFAULT_ENTITIES:,})",
    )
    args = parser.parse_args()
    cfg = BenchConfig.from_args(args)

    print_system_info()

    entity_count = args.entities

    # Warmup bytecode VM
    print("Warming up bytecode VM...", flush=True)
    warmup_app = _make_increment_app(10)
    warmup_app.update()
    print()

    print("=" * 80)
    title = (
        f"View API Workloads ({entity_count:,} entities, "
        f"{cfg.warmup} warmup, {cfg.iterations} iter, {cfg.rounds} rounds)"
    )
    print(title)
    print("=" * 80)
    print(
        f"{'Workload':<18}  {'Description':<25}  {'Median +/- IQR':>22}"
    )
    print("-" * 80)

    for name, desc, factory in WORKLOADS:
        print(f"  Benchmarking {name}...", end="", flush=True)
        app = factory(entity_count)
        result = _bench_app(app, cfg)
        print(f" {result.median_ms:.3f}ms")
        print(f"{name:<18}  {desc:<25}  {result.format_compact():>22}")

    print("=" * 80)
    print()
    print("For static vs dynamic bridge dispatch comparison, run:")
    print("  cargo bench --bench query_dispatch")
    print()


if __name__ == "__main__":
    main()
