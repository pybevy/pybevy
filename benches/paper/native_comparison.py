#!/usr/bin/env python3
"""Paper Table 3: PyBevy tiers vs native Bevy at 1M entities.

Compares all 3 PyBevy execution tiers (Query, View API bytecode VM, Numba)
against native Bevy baseline from `cargo bench --bench vm_overhead`.

Usage:
    poetry run python benches/paper/native_comparison.py
    poetry run python benches/paper/native_comparison.py --entities 1000000
    poetry run python benches/paper/native_comparison.py --rounds 1 --iterations 3  # quick test

Requirements:
    - PyBevy built in release mode: poetry run maturin develop --release
    - Numba: poetry install -E numba
    - Native baseline: cargo bench --bench vm_overhead (update NATIVE_BEVY_MS below)
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
from pybevy.app import (
    App,
    RunMode,
    ScheduleRunnerPlugin,
    Startup,
    Update,
)
from pybevy.ecs import Commands, Mut, Query, View
from pybevy.expr import cos, sin
from pybevy.transform import Transform

# Native Bevy baseline @ 1M entities (from cargo bench --bench vm_overhead)
# Update this value after running: cargo bench --bench vm_overhead
NATIVE_BEVY_1M_MS = 0.513

# Native Bevy physics baseline @ 1M entities (from cargo bench --bench native_physics)
# Update this value after running: cargo bench --bench native_physics
# Use the physics_par_iter/par_iter/1000000 result
NATIVE_PHYSICS_1M_MS = 0.672

DEFAULT_ENTITIES = 1_000_000


# =============================================================================
# Numba Kernel
# =============================================================================


def _get_increment_column():  # type: ignore[no-untyped-def]
    import numba  # type: ignore[import-untyped]

    @numba.jit(nopython=True)
    def increment_column(col):  # type: ignore[no-untyped-def]
        for i in range(len(col)):
            col[i] = col[i] + 1.0

    return increment_column


# =============================================================================
# App Factories
# =============================================================================


def _make_query_app(entity_count: int) -> App:
    """Query iteration: Python for-loop over entities."""

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
    """View API: bytecode VM batch execution."""

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
    """ViewColumn + Numba: JIT-compiled native loop."""
    jit_increment = _get_increment_column()

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), 0.0, 0.0))

    def increment(view: View[Mut[Transform]]) -> None:
        for batch in view.iter_batches():
            col = batch.column_mut(Transform)
            jit_increment(col.translation.x)

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, increment)
    app.initialize()
    return app


# =============================================================================
# App Factories (Physics Workload)
# =============================================================================


def _get_physics_column():  # type: ignore[no-untyped-def]
    import math

    import numba  # type: ignore[import-untyped]

    @numba.jit(nopython=True, parallel=True)
    def physics_column(pos_x, pos_y, pos_z, dt):  # type: ignore[no-untyped-def]
        for i in numba.prange(len(pos_x)):
            x = pos_x[i]
            y = pos_y[i]
            z = pos_z[i]
            pos_x[i] = x + y * dt + 0.5 * y * y * dt
            pos_y[i] = y + math.sin(x * 0.1) * dt
            pos_z[i] = z + math.cos(x * 0.1) * dt

    return physics_column


def _make_query_physics_app(entity_count: int) -> App:
    """Query iteration: Python for-loop with trig physics."""
    import math

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


def _make_view_physics_app(entity_count: int) -> App:
    """View API: bytecode VM with trig physics."""

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


def _make_numba_physics_app(entity_count: int) -> App:
    """ViewColumn + Numba: JIT-compiled physics kernel."""
    jit_physics = _get_physics_column()

    def setup(commands: Commands) -> None:
        for i in range(entity_count):
            commands.spawn(Transform.from_xyz(float(i), float(i) * 0.5, 0.0))

    def physics(view: View[Mut[Transform]]) -> None:
        for batch in view.iter_batches():
            col = batch.column_mut(Transform)
            jit_physics(
                col.translation.x, col.translation.y, col.translation.z, 0.016
            )

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, physics)
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


def main() -> None:
    """Run native comparison benchmark."""
    parser = argparse.ArgumentParser(
        description="Paper Table 3: PyBevy tiers vs native Bevy"
    )
    add_bench_args(parser)
    parser.add_argument(
        "--entities",
        type=int,
        default=DEFAULT_ENTITIES,
        help=f"Entity count (default: {DEFAULT_ENTITIES:,})",
    )
    parser.add_argument(
        "--native-ms",
        type=float,
        default=NATIVE_BEVY_1M_MS,
        help=f"Native Bevy baseline in ms (default: {NATIVE_BEVY_1M_MS})",
    )
    parser.add_argument(
        "--native-physics-ms",
        type=float,
        default=NATIVE_PHYSICS_1M_MS,
        help=f"Native Bevy physics baseline in ms (default: {NATIVE_PHYSICS_1M_MS})",
    )
    parser.add_argument(
        "--skip-query",
        action="store_true",
        help="Skip Query benchmark (very slow at 1M entities)",
    )
    args = parser.parse_args()
    cfg = BenchConfig.from_args(args)
    native_ms = args.native_ms
    native_physics_ms = args.native_physics_ms

    print_system_info()

    # Check Numba availability
    try:
        import numba  # type: ignore[import-untyped]  # noqa: F401

        has_numba = True
    except ImportError:
        has_numba = False
        print("WARNING: Numba not available. Numba tier will be skipped.")
        print()

    # Warmup
    if has_numba:
        print("Warming up Numba JIT...", flush=True)
        warmup_app = _make_numba_app(10)
        warmup_app.update()

    print("Warming up View bytecode...", flush=True)
    warmup_view = _make_view_app(10)
    warmup_view.update()
    print()

    entity_count = args.entities
    scale_factor = entity_count / 1_000_000  # Scale native baseline if not 1M

    print("=" * 80)
    title = (
        f"Native Comparison ({entity_count:,} entities, "
        f"{cfg.warmup} warmup, {cfg.iterations} iter, {cfg.rounds} rounds)"
    )
    print(title)
    print("=" * 80)
    print(
        f"{'Approach':<28}  {'Median +/- IQR':>22}  {'vs Native':>10}  {'Efficiency':>10}"
    )
    print("-" * 80)

    # Native baseline (from cargo bench)
    scaled_native_ms = native_ms * scale_factor
    print(
        f"{'Native Bevy (Rust)':<28}  {scaled_native_ms:>18.2f}    ms  {'1.0x':>10}  {'100%':>10}"
    )

    # ViewColumn + Numba
    if has_numba:
        print("  Benchmarking Numba...", end="", flush=True)
        numba_app = _make_numba_app(entity_count)
        numba_result = _bench_app(numba_app, cfg)
        print(f" {numba_result.median_ms:.3f}ms")
        numba_ratio = numba_result.median_ms / scaled_native_ms
        numba_eff = 100.0 / numba_ratio
        print(
            f"{'ViewColumn + Numba':<28}  {numba_result.format_compact():>22}"
            f"  {numba_ratio:>8.1f}x  {numba_eff:>9.0f}%"
        )
    else:
        print(f"{'ViewColumn + Numba':<28}  {'(numba not available)':>22}  {'--':>10}  {'--':>10}")

    # View API (bytecode VM)
    print("  Benchmarking View API...", end="", flush=True)
    view_app = _make_view_app(entity_count)
    view_result = _bench_app(view_app, cfg)
    print(f" {view_result.median_ms:.3f}ms")
    view_ratio = view_result.median_ms / scaled_native_ms
    view_eff = 100.0 / view_ratio
    print(
        f"{'View API (bytecode VM)':<28}  {view_result.format_compact():>22}"
        f"  {view_ratio:>8.1f}x  {view_eff:>9.0f}%"
    )

    # Query iteration
    if not args.skip_query:
        print("  Benchmarking Query...", end="", flush=True)
        query_app = _make_query_app(entity_count)
        query_result = _bench_app(query_app, cfg)
        print(f" {query_result.median_ms:.3f}ms")
        query_ratio = query_result.median_ms / scaled_native_ms
        query_eff = 100.0 / query_ratio
        print(
            f"{'Query iteration':<28}  {query_result.format_compact():>22}"
            f"  {query_ratio:>8.0f}x  {query_eff:>8.2f}%"
        )
    else:
        print(f"{'Query iteration':<28}  {'(skipped)':>22}  {'--':>10}  {'--':>10}")

    print("=" * 80)
    print()
    print("Workload: increment Transform.translation.x by 1.0")
    print()

    # =========================================================================
    # Physics Workload
    # =========================================================================

    print("=" * 80)
    title_phys = (
        f"Native Comparison - Physics ({entity_count:,} entities, "
        f"{cfg.warmup} warmup, {cfg.iterations} iter, {cfg.rounds} rounds)"
    )
    print(title_phys)
    print("=" * 80)
    print(
        f"{'Approach':<28}  {'Median +/- IQR':>22}  {'vs Native':>10}  {'Efficiency':>10}"
    )
    print("-" * 80)

    # Native physics baseline (from cargo bench)
    scaled_native_phys_ms = native_physics_ms * scale_factor
    print(
        f"{'Native Bevy (Rust)':<28}  {scaled_native_phys_ms:>18.2f}    ms  {'1.0x':>10}  {'100%':>10}"
    )

    # ViewColumn + Numba (physics)
    if has_numba:
        print("  Benchmarking Numba physics...", end="", flush=True)
        numba_phys_app = _make_numba_physics_app(entity_count)
        numba_phys_result = _bench_app(numba_phys_app, cfg)
        print(f" {numba_phys_result.median_ms:.3f}ms")

    # View API (physics)
    print("  Benchmarking View API physics...", end="", flush=True)
    view_phys_app = _make_view_physics_app(entity_count)
    view_phys_result = _bench_app(view_phys_app, cfg)
    print(f" {view_phys_result.median_ms:.3f}ms")

    # Query iteration (physics) - may be very slow at 1M
    if not args.skip_query:
        print("  Benchmarking Query physics...", end="", flush=True)
        query_phys_app = _make_query_physics_app(entity_count)
        query_phys_result = _bench_app(query_phys_app, cfg)
        print(f" {query_phys_result.median_ms:.3f}ms")

        query_phys_ratio = query_phys_result.median_ms / scaled_native_phys_ms
        query_phys_eff = 100.0 / query_phys_ratio
        print(
            f"{'Query iteration':<28}  {query_phys_result.format_compact():>22}"
            f"  {query_phys_ratio:>8.0f}x  {query_phys_eff:>8.2f}%"
        )
    else:
        query_phys_result = None
        print(f"{'Query iteration':<28}  {'(skipped)':>22}  {'--':>10}  {'--':>10}")

    view_phys_ratio = view_phys_result.median_ms / scaled_native_phys_ms
    view_phys_eff = 100.0 / view_phys_ratio
    print(
        f"{'View API (bytecode VM)':<28}  {view_phys_result.format_compact():>22}"
        f"  {view_phys_ratio:>8.1f}x  {view_phys_eff:>9.0f}%"
    )

    if has_numba:
        numba_phys_ratio = numba_phys_result.median_ms / scaled_native_phys_ms
        numba_phys_eff = 100.0 / numba_phys_ratio
        print(
            f"{'ViewColumn + Numba':<28}  {numba_phys_result.format_compact():>22}"
            f"  {numba_phys_ratio:>8.1f}x  {numba_phys_eff:>9.0f}%"
        )
    else:
        print(f"{'ViewColumn + Numba':<28}  {'(numba not available)':>22}  {'--':>10}  {'--':>10}")

    print("=" * 80)
    print()
    print(f"Native baseline (increment): {native_ms}ms @ 1M entities (cargo bench --bench vm_overhead)")
    print(f"Native baseline (physics): {native_physics_ms}ms @ 1M entities (cargo bench --bench native_physics)")
    print("Physics kernel: pos += vel*dt + 0.5*vel^2*dt, vel += sin/cos(pos*0.1)*dt")
    print()


if __name__ == "__main__":
    main()
