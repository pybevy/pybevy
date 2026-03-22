"""NumPy flat-array benchmarks for paper comparison.

Measures NumPy performance on contiguous float32 arrays to compare
against PyBevy's View API bytecode VM and Numba tiers. Two workloads:

1. Trivial increment: x,y,z += 1.0 (3 arrays x N float32, matching PyBevy scaling benchmark)
2. Physics kernel: pos/vel update with sin/cos (6 arrays x N float32, matching PyBevy physics)

This isolates how much of PyBevy's batch-tier speedup comes from
avoiding CPython interpretation vs the ECS columnar layout.
"""

from __future__ import annotations

import argparse
import sys

import numpy as np

sys.path.insert(0, "benches/paper")
from bench_utils import (  # type: ignore[import-not-found]
    BenchConfig,
    add_bench_args,
    print_system_info,
    print_table_footer,
    print_table_header,
    run_benchmark,
)


def bench_increment(n: int, config: BenchConfig) -> None:
    """Benchmark 3-field increment: x,y,z += 1.0 on float32 arrays.

    Matches PyBevy's scaling benchmark (Table 2): Transform x,y,z += 1.0
    """
    x = np.ones(n, dtype=np.float32)
    y = np.ones(n, dtype=np.float32)
    z = np.ones(n, dtype=np.float32)

    def fn() -> None:
        x[:] += 1.0
        y[:] += 1.0
        z[:] += 1.0

    result = run_benchmark(fn, config.warmup, config.iterations, config.rounds)

    print_table_header(
        f"NumPy Increment x,y,z += 1.0 (n={n:,})",
        ["Operation", "Time"],
        config.warmup,
        config.iterations,
        config.rounds,
    )
    print(f"  {'x,y,z += 1.0':>20}  {result.format_compact():>20}")
    print_table_footer()


def bench_physics(n: int, config: BenchConfig) -> None:
    """Benchmark physics kernel: pos/vel update with sin/cos.

    Matches the PyBevy physics benchmark kernel:
      pos.x += vel.x * dt + 0.5 * vel.x**2 * dt
      pos.y += vel.y * dt + 0.5 * vel.y**2 * dt
      pos.z += vel.z * dt + 0.5 * vel.z**2 * dt
      vel.x += sin(pos.x * 0.1) * dt
      vel.y += cos(pos.y * 0.1) * dt
      vel.z += sin(pos.z * 0.1) * cos(pos.z * 0.1) * dt
    """
    rng = np.random.default_rng(42)
    pos_x = rng.random(n, dtype=np.float32)
    pos_y = rng.random(n, dtype=np.float32)
    pos_z = rng.random(n, dtype=np.float32)
    vel_x = rng.random(n, dtype=np.float32)
    vel_y = rng.random(n, dtype=np.float32)
    vel_z = rng.random(n, dtype=np.float32)
    dt = np.float32(0.016)

    def fn() -> None:
        pos_x[:] += vel_x * dt + 0.5 * vel_x**2 * dt
        pos_y[:] += vel_y * dt + 0.5 * vel_y**2 * dt
        pos_z[:] += vel_z * dt + 0.5 * vel_z**2 * dt
        vel_x[:] += np.sin(pos_x * 0.1) * dt
        vel_y[:] += np.cos(pos_y * 0.1) * dt
        vel_z[:] += np.sin(pos_z * 0.1) * np.cos(pos_z * 0.1) * dt

    result = run_benchmark(fn, config.warmup, config.iterations, config.rounds)

    print_table_header(
        f"NumPy Physics Kernel (n={n:,})",
        ["Operation", "Time"],
        config.warmup,
        config.iterations,
        config.rounds,
    )
    print(f"  {'physics kernel':>20}  {result.format_compact():>20}")
    print_table_footer()


def main() -> None:
    parser = argparse.ArgumentParser(description="NumPy flat-array benchmarks")
    add_bench_args(parser)
    parser.add_argument(
        "--elements",
        type=int,
        default=1_000_000,
        help="Number of float32 elements (default: 1000000)",
    )
    args = parser.parse_args()
    config = BenchConfig.from_args(args)

    print_system_info()
    print(f"NumPy version: {np.__version__}")
    print(f"Elements: {args.elements:,}")
    print()

    bench_increment(args.elements, config)
    bench_physics(args.elements, config)


if __name__ == "__main__":
    main()
