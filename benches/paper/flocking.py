#!/usr/bin/env python3
"""Paper Table: Flocking (boids) simulation benchmark.

A realistic workload with O(n^2) neighbor interactions per entity.
Each boid applies three steering rules (separation, alignment, cohesion)
based on nearby neighbors within a perception radius. This tests:
- Data-dependent branching (neighbor within radius?)
- Cross-entity reads (each boid reads all others' positions/velocities)
- Per-entity mutable writes (update own velocity/position)

The View API (bytecode VM) cannot express neighbor lookups, so only
Query (Python) and Numba JIT are compared. This demonstrates a workload
where the batch expression path is inapplicable and Numba is essential.

Usage:
    poetry run python benches/paper/flocking.py
    poetry run python benches/paper/flocking.py --counts 500 1000 2000
    poetry run python benches/paper/flocking.py --rounds 1 --iterations 3  # quick smoke test

Requirements:
    - PyBevy built in release mode: poetry run maturin develop --release
    - Numba: poetry install -E numba
"""

from __future__ import annotations

import argparse
import math
import statistics
import time
from dataclasses import dataclass, field

from benches.paper.bench_utils import (
    BenchConfig,
    BenchResult,
    add_bench_args,
    compute_speedup,
    print_system_info,
)
from pybevy.app import App, RunMode, ScheduleRunnerPlugin, Startup, Update
from pybevy.decorators import component
from pybevy.ecs import Commands, Mut, Query, View
from pybevy.math import Vec3
from pybevy.prelude import Component
from pybevy.transform import Transform

DEFAULT_COUNTS = [500, 1_000, 2_000, 5_000]

# Flocking parameters
PERCEPTION_RADIUS = 5.0
SEPARATION_WEIGHT = 1.5
ALIGNMENT_WEIGHT = 1.0
COHESION_WEIGHT = 1.0
MAX_SPEED = 2.0
MAX_FORCE = 0.5
DT = 0.016
BOUND = 50.0


@component
@dataclass
class Velocity(Component):
    vel: Vec3 = field(default_factory=lambda: Vec3.ZERO)


# =============================================================================
# Query (Python) implementation
# =============================================================================


def _make_query_app(entity_count: int) -> App:
    """Create an app with Query-based flocking."""

    def setup(commands: Commands) -> None:
        import random

        random.seed(42)
        for _ in range(entity_count):
            x = random.uniform(-BOUND, BOUND)
            y = random.uniform(-BOUND, BOUND)
            z = 0.0
            vx = random.uniform(-1.0, 1.0)
            vy = random.uniform(-1.0, 1.0)
            commands.spawn(
                Transform.from_xyz(x, y, z),
                Velocity(vel=Vec3(vx, vy, 0.0)),
            )

    def flocking(
        query: Query[tuple[Mut[Transform], Mut[Velocity]]],
    ) -> None:
        # Single iteration: read state, compute flocking, write back
        # Query.__iter__ returns self, so it can only be iterated once.
        # We collect ECS data, compute the O(n^2) flocking in pure Python,
        # and write results back via the stored references.
        transforms: list[Transform] = []
        vels: list[Velocity] = []
        positions: list[tuple[float, float]] = []
        velocities: list[tuple[float, float]] = []
        for t, v in query:
            transforms.append(t)
            vels.append(v)
            positions.append((t.translation.x, t.translation.y))
            velocities.append((v.vel.x, v.vel.y))

        n = len(positions)
        r2 = PERCEPTION_RADIUS * PERCEPTION_RADIUS

        # Compute flocking forces (O(n^2) neighbor search)
        for i in range(n):
            px, py = positions[i]

            # Accumulate steering forces
            sep_x, sep_y = 0.0, 0.0
            ali_x, ali_y = 0.0, 0.0
            coh_x, coh_y = 0.0, 0.0
            count = 0

            for j in range(n):
                if j == i:
                    continue
                dx = positions[j][0] - px
                dy = positions[j][1] - py
                d2 = dx * dx + dy * dy
                if d2 < r2 and d2 > 0.0001:
                    d = math.sqrt(d2)
                    # Separation: steer away from close neighbors
                    sep_x -= dx / d
                    sep_y -= dy / d
                    # Alignment: match neighbor velocity
                    ali_x += velocities[j][0]
                    ali_y += velocities[j][1]
                    # Cohesion: steer toward center of neighbors
                    coh_x += dx
                    coh_y += dy
                    count += 1

            t = transforms[i]
            v = vels[i]

            if count > 0:
                inv_count = 1.0 / count
                # Apply weights
                fx = (
                    sep_x * SEPARATION_WEIGHT
                    + ali_x * inv_count * ALIGNMENT_WEIGHT
                    + coh_x * inv_count * COHESION_WEIGHT
                )
                fy = (
                    sep_y * SEPARATION_WEIGHT
                    + ali_y * inv_count * ALIGNMENT_WEIGHT
                    + coh_y * inv_count * COHESION_WEIGHT
                )

                # Clamp force
                f_mag = math.sqrt(fx * fx + fy * fy)
                if f_mag > MAX_FORCE:
                    fx = fx / f_mag * MAX_FORCE
                    fy = fy / f_mag * MAX_FORCE

                v.vel.x += fx * DT  # type: ignore[union-attr]
                v.vel.y += fy * DT  # type: ignore[union-attr]

            # Clamp speed
            vx = v.vel.x  # type: ignore[union-attr]
            vy = v.vel.y  # type: ignore[union-attr]
            speed = math.sqrt(vx * vx + vy * vy)
            if speed > MAX_SPEED:
                v.vel.x = vx / speed * MAX_SPEED  # type: ignore[union-attr]
                v.vel.y = vy / speed * MAX_SPEED  # type: ignore[union-attr]
                vx = v.vel.x  # type: ignore[union-attr]
                vy = v.vel.y  # type: ignore[union-attr]

            # Update position
            t.translation.x += vx * DT  # type: ignore[union-attr]
            t.translation.y += vy * DT  # type: ignore[union-attr]

            # Wrap around bounds
            if t.translation.x > BOUND:  # type: ignore[union-attr]
                t.translation.x -= 2 * BOUND  # type: ignore[union-attr]
            elif t.translation.x < -BOUND:  # type: ignore[union-attr]
                t.translation.x += 2 * BOUND  # type: ignore[union-attr]
            if t.translation.y > BOUND:  # type: ignore[union-attr]
                t.translation.y -= 2 * BOUND  # type: ignore[union-attr]
            elif t.translation.y < -BOUND:  # type: ignore[union-attr]
                t.translation.y += 2 * BOUND  # type: ignore[union-attr]

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, flocking)
    app.initialize()
    return app


# =============================================================================
# Numba JIT implementation
# =============================================================================


def _get_numba_flocking(parallel: bool = True):  # type: ignore[no-untyped-def]
    import numba  # type: ignore[import-untyped]
    import numpy as np  # type: ignore[import-untyped]

    @numba.jit(nopython=True, parallel=parallel)
    def numba_flocking(  # type: ignore[no-untyped-def]
        translation, vel, n,
        perception_r2, sep_w, ali_w, coh_w, max_speed, max_force, dt, bound
    ):
        # Pre-read positions and velocities into contiguous arrays
        # (ViewColumn may have non-unit stride, so copy once for O(n^2) reads)
        px = np.empty(n, dtype=np.float64)
        py = np.empty(n, dtype=np.float64)
        vvx = np.empty(n, dtype=np.float64)
        vvy = np.empty(n, dtype=np.float64)
        for i in range(n):
            px[i] = translation.x[i]
            py[i] = translation.y[i]
            vvx[i] = vel.x[i]
            vvy[i] = vel.y[i]

        for i in numba.prange(n):
            pxi = px[i]
            pyi = py[i]

            sep_x = 0.0
            sep_y = 0.0
            ali_x = 0.0
            ali_y = 0.0
            coh_x = 0.0
            coh_y = 0.0
            count = 0

            for j in range(n):
                if j == i:
                    continue
                dx = px[j] - pxi
                dy = py[j] - pyi
                d2 = dx * dx + dy * dy
                if d2 < perception_r2 and d2 > 0.0001:
                    d = math.sqrt(d2)
                    sep_x -= dx / d
                    sep_y -= dy / d
                    ali_x += vvx[j]
                    ali_y += vvy[j]
                    coh_x += dx
                    coh_y += dy
                    count += 1

            new_vx = vvx[i]
            new_vy = vvy[i]

            if count > 0:
                inv_count = 1.0 / count
                fx = (
                    sep_x * sep_w
                    + ali_x * inv_count * ali_w
                    + coh_x * inv_count * coh_w
                )
                fy = (
                    sep_y * sep_w
                    + ali_y * inv_count * ali_w
                    + coh_y * inv_count * coh_w
                )

                f_mag = math.sqrt(fx * fx + fy * fy)
                if f_mag > max_force:
                    fx = fx / f_mag * max_force
                    fy = fy / f_mag * max_force

                new_vx += fx * dt
                new_vy += fy * dt

            speed = math.sqrt(new_vx * new_vx + new_vy * new_vy)
            if speed > max_speed:
                new_vx = new_vx / speed * max_speed
                new_vy = new_vy / speed * max_speed

            vel.x[i] = new_vx
            vel.y[i] = new_vy

            new_px = pxi + new_vx * dt
            new_py = pyi + new_vy * dt

            if new_px > bound:
                new_px -= 2.0 * bound
            elif new_px < -bound:
                new_px += 2.0 * bound
            if new_py > bound:
                new_py -= 2.0 * bound
            elif new_py < -bound:
                new_py += 2.0 * bound

            translation.x[i] = new_px
            translation.y[i] = new_py

    return numba_flocking


def _make_numba_app(entity_count: int, parallel: bool = True) -> App:
    """Create an app with Numba JIT flocking."""
    jit_flocking = _get_numba_flocking(parallel=parallel)
    r2 = PERCEPTION_RADIUS * PERCEPTION_RADIUS

    def setup(commands: Commands) -> None:
        import random

        random.seed(42)
        for _ in range(entity_count):
            x = random.uniform(-BOUND, BOUND)
            y = random.uniform(-BOUND, BOUND)
            vx = random.uniform(-1.0, 1.0)
            vy = random.uniform(-1.0, 1.0)
            commands.spawn(
                Transform.from_xyz(x, y, 0.0),
                Velocity(vel=Vec3(vx, vy, 0.0)),
            )

    def flocking(view: View[tuple[Mut[Transform], Mut[Velocity]]]) -> None:
        for batch in view.iter_batches():
            pos = batch.column_mut(Transform)
            vel = batch.column_mut(Velocity)
            n = len(pos.translation.x)
            jit_flocking(
                pos.translation, vel.vel, n,
                r2, SEPARATION_WEIGHT, ALIGNMENT_WEIGHT, COHESION_WEIGHT,
                MAX_SPEED, MAX_FORCE, DT, BOUND,
            )

    app = App()
    app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
    app.add_systems(Startup, setup)
    app.add_systems(Update, flocking)
    app.initialize()
    return app


# =============================================================================
# Benchmark Runner
# =============================================================================


def _bench_app(app: App, cfg: BenchConfig) -> BenchResult:
    """Run benchmark on a pre-initialized app."""
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
    """Run flocking workload benchmark."""
    parser = argparse.ArgumentParser(
        description="Paper Table: Flocking (boids) simulation benchmark"
    )
    add_bench_args(parser)
    parser.add_argument(
        "--counts",
        type=int,
        nargs="+",
        default=DEFAULT_COUNTS,
        help=f"Entity counts (default: {DEFAULT_COUNTS})",
    )
    args = parser.parse_args()
    cfg = BenchConfig.from_args(args)

    print_system_info()

    # Check Numba
    try:
        import numba  # type: ignore[import-untyped]  # noqa: F401

        has_numba = True
    except ImportError:
        has_numba = False
        print("WARNING: Numba not available. Numba results will be skipped.")
        print()

    if has_numba:
        print("Warming up Numba JIT (parallel kernel)...", flush=True)
        warmup_app = _make_numba_app(10, parallel=True)
        warmup_app.update()
        print("Warming up Numba JIT (single-thread kernel)...", flush=True)
        warmup_app_st = _make_numba_app(10, parallel=False)
        warmup_app_st.update()
        print()

    w = 22
    print("=" * 120)
    title = (
        f"Flocking Simulation ({cfg.warmup} warmup, "
        f"{cfg.iterations} iter, {cfg.rounds} rounds)"
    )
    print(title)
    print("=" * 120)
    print(f"  Perception radius: {PERCEPTION_RADIUS}, "
          f"Bounds: +/-{BOUND}, O(n^2) neighbor search")
    print()
    hdr = f"{'Entities':>12}  {'Query (Python)':>{w}}"
    if has_numba:
        hdr += f"  {'Numba (1 thread)':>{w}}  {'vs Query':>10}"
        hdr += f"  {'Numba (parallel)':>{w}}  {'vs Query':>10}"
    print(hdr)
    print("-" * 120)

    for count in sorted(args.counts):
        row = f"{count:>12,}"

        # Query
        print(f"  Benchmarking Query @ {count:,}...", end="", flush=True)
        query_app = _make_query_app(count)
        query_result = _bench_app(query_app, cfg)
        print(f" {query_result.median_ms:.1f}ms")
        row += f"  {query_result.format_compact():>{w}}"

        # Numba single-threaded
        if has_numba:
            print(f"  Benchmarking Numba (1T) @ {count:,}...", end="", flush=True)
            numba_st_app = _make_numba_app(count, parallel=False)
            numba_st_result = _bench_app(numba_st_app, cfg)
            print(f" {numba_st_result.median_ms:.1f}ms")
            row += f"  {numba_st_result.format_compact():>{w}}"
            row += f"  {compute_speedup(query_result, numba_st_result):>10}"

            # Numba parallel
            print(f"  Benchmarking Numba (par) @ {count:,}...", end="", flush=True)
            numba_par_app = _make_numba_app(count, parallel=True)
            numba_par_result = _bench_app(numba_par_app, cfg)
            print(f" {numba_par_result.median_ms:.1f}ms")
            row += f"  {numba_par_result.format_compact():>{w}}"
            row += f"  {compute_speedup(query_result, numba_par_result):>10}"

        print(row)

    print("=" * 120)
    print()
    print("Workload: O(n^2) boids flocking (separation + alignment + cohesion)")
    print("Note: View API (bytecode VM) cannot express neighbor lookups")
    print("      and is not included in this benchmark.")
    print()


if __name__ == "__main__":
    main()
