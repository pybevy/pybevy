#!/usr/bin/env python3
"""Paper Table 4: Physics workload benchmark.

Compares Query iteration vs ViewColumn+Numba on a complex physics kernel
(trigonometric computation per entity) at 5K entities.

Usage:
    poetry run python benches/paper/physics.py
    poetry run python benches/paper/physics.py --entities 10000
    poetry run python benches/paper/physics.py --rounds 1 --iterations 3  # quick smoke test

Requirements:
    - PyBevy built in release mode: poetry run maturin develop --release
    - Numba: poetry install -E numba
"""

from __future__ import annotations

import argparse
import math
import statistics
import time

from benches.paper.bench_utils import (
    BenchConfig,
    BenchResult,
    add_bench_args,
    compute_speedup,
    print_system_info,
)
from pybevy.app import App, RunMode, ScheduleRunnerPlugin, Startup, Update
from pybevy.ecs import Commands, Mut, Query, View
from pybevy.expr import cos, sin
from pybevy.transform import Transform

DEFAULT_ENTITIES = 5_000




def _get_numba_physics():  # type: ignore[no-untyped-def]
    import numba  # type: ignore[import-untyped]

    @numba.jit(nopython=True, parallel=True)
    def numba_physics(translation, dt):  # type: ignore[no-untyped-def]
        for i in numba.prange(len(translation.x)):
            x = translation.x[i]
            y = translation.y[i]
            z = translation.z[i]
            translation.x[i] = x + y * dt + 0.5 * y * y * dt
            translation.y[i] = y + math.sin(x * 0.1) * dt
            translation.z[i] = z + math.cos(x * 0.1) * dt

    return numba_physics




def _make_query_app(entity_count: int) -> App:
    """Create an app with Query-based physics system."""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), float(i) * 0.5, 0.0))

    def physics(query: Query[Mut[Transform]]) -> None:
        dt = 0.016
        for t in query:
            x = t.translation.x
            y = t.translation.y
            z = t.translation.z
            t.translation.x = x + y * dt + 0.5 * y * y * dt
            t.translation.y = y + math.sin(x * 0.1) * dt
            t.translation.z = z + math.cos(x * 0.1) * dt

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, physics)
    app.initialize()
    return app


def _make_view_app(entity_count: int) -> App:
    """Create an app with View API (bytecode VM) physics system."""

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), float(i) * 0.5, 0.0))

    def physics(view: View[Mut[Transform]]) -> None:
        col = view.column_mut(Transform)
        dt = 0.016
        x = col.translation.x
        y = col.translation.y
        z = col.translation.z
        col.translation.x = x + y * dt + 0.5 * y * y * dt
        col.translation.y = y + sin(x * 0.1) * dt
        col.translation.z = z + cos(x * 0.1) * dt

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, physics)
    app.initialize()
    return app


def _make_numba_app(entity_count: int) -> App:
    """Create an app with ViewColumn+Numba physics system."""
    jit_physics = _get_numba_physics()

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), float(i) * 0.5, 0.0))

    def physics(view: View[Mut[Transform]]) -> None:
        for batch in view.iter_batches():
            col = batch.column_mut(Transform)
            jit_physics(col.translation, 0.016)

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, physics)
    app.initialize()
    return app




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




def main() -> None:
    """Run physics workload benchmark."""
    parser = argparse.ArgumentParser(
        description="Paper Table 4: Physics workload benchmark"
    )
    add_bench_args(parser)
    parser.add_argument(
        "--entities",
        type=int,
        default=DEFAULT_ENTITIES,
        help=f"Entity count (default: {DEFAULT_ENTITIES})",
    )
    parser.add_argument(
        "--native-physics-ms",
        type=float,
        default=None,
        help="Native Bevy physics baseline in ms at 1M entities (from cargo bench --bench native_physics)",
    )
    args = parser.parse_args()
    cfg = BenchConfig.from_args(args)
    native_physics_ms: float | None = args.native_physics_ms

    print_system_info()

    # Check Numba availability
    try:
        import numba  # type: ignore[import-untyped]  # noqa: F401

        has_numba = True
    except ImportError:
        has_numba = False
        print("WARNING: Numba not available. Numba results will be skipped.")
        print()

    # Warmup View bytecode VM
    print("Warming up View bytecode VM...", flush=True)
    warmup_view = _make_view_app(10)
    warmup_view.update()

    if has_numba:
        print("Warming up Numba JIT...", flush=True)
        warmup_app = _make_numba_app(10)
        warmup_app.update()
        print()

    entity_count = args.entities

    # Compute scaled native baseline if provided
    scaled_native_ms: float | None = None
    if native_physics_ms is not None:
        scale_factor = entity_count / 1_000_000
        scaled_native_ms = native_physics_ms * scale_factor

    print("=" * 80)
    title = (
        f"Physics Workload ({entity_count:,} entities, "
        f"{cfg.warmup} warmup, {cfg.iterations} iter, {cfg.rounds} rounds)"
    )
    print(title)
    print("=" * 80)
    if scaled_native_ms is not None:
        print(f"{'Approach':<25}  {'Median +/- IQR':>22}  {'Min':>10}  {'Max':>10}  {'vs Native':>10}")
    else:
        print(f"{'Approach':<25}  {'Median +/- IQR':>22}  {'Min':>10}  {'Max':>10}")
    print("-" * 80)

    # Native baseline row (if provided)
    if scaled_native_ms is not None:
        print(
            f"{'Native Bevy (Rust)':<25}  {scaled_native_ms:>18.2f}    ms  {'--':>10}  {'--':>10}  {'1.0x':>10}"
        )

    # Query benchmark
    print("  Benchmarking Query...", end="", flush=True)
    query_app = _make_query_app(entity_count)
    query_result = _bench_app(query_app, cfg)
    print(f" {query_result.median_ms:.3f}ms")

    query_vs_native = f"  {query_result.median_ms / scaled_native_ms:>8.0f}x" if scaled_native_ms else ""
    print(
        f"{'Query iteration':<25}  {query_result.format_compact():>22}"
        f"  {query_result.min_ms:>8.3f}ms  {query_result.max_ms:>8.3f}ms{query_vs_native}"
    )

    # View API benchmark
    print("  Benchmarking View API...", end="", flush=True)
    view_app = _make_view_app(entity_count)
    view_result = _bench_app(view_app, cfg)
    print(f" {view_result.median_ms:.3f}ms")

    view_vs_native = f"  {view_result.median_ms / scaled_native_ms:>8.1f}x" if scaled_native_ms else ""
    print(
        f"{'View API (bytecode VM)':<25}  {view_result.format_compact():>22}"
        f"  {view_result.min_ms:>8.3f}ms  {view_result.max_ms:>8.3f}ms{view_vs_native}"
    )

    # Numba benchmark
    if has_numba:
        print("  Benchmarking Numba...", end="", flush=True)
        numba_app = _make_numba_app(entity_count)
        numba_result = _bench_app(numba_app, cfg)
        print(f" {numba_result.median_ms:.3f}ms")

        numba_vs_native = f"  {numba_result.median_ms / scaled_native_ms:>8.1f}x" if scaled_native_ms else ""
        print(
            f"{'ViewColumn+Numba':<25}  {numba_result.format_compact():>22}"
            f"  {numba_result.min_ms:>8.3f}ms  {numba_result.max_ms:>8.3f}ms{numba_vs_native}"
        )
        print("-" * 80)
        print(f"{'View vs Query':<25}  {compute_speedup(query_result, view_result):>22}")
        print(f"{'Numba vs Query':<25}  {compute_speedup(query_result, numba_result):>22}")
        print(f"{'Numba vs View':<25}  {compute_speedup(view_result, numba_result):>22}")
    else:
        print("-" * 80)
        print(f"{'View vs Query':<25}  {compute_speedup(query_result, view_result):>22}")
        print(f"{'ViewColumn+Numba':<25}  {'(numba not available)':>22}")

    print("=" * 80)
    print()
    print("Kernel: pos += vel*dt + 0.5*vel^2*dt, vel += sin/cos(pos*0.1)*dt")
    if native_physics_ms is not None:
        print(f"Native baseline (physics): {native_physics_ms}ms @ 1M entities (cargo bench --bench native_physics)")
    print()


if __name__ == "__main__":
    main()
