"""Shared benchmark infrastructure for reproducible paper benchmarks.

Provides system information collection, statistical framework with
configurable warmup/measured iterations/rounds, and formatted output.
"""

from __future__ import annotations

import argparse
import os
import platform
import statistics
import time
from collections.abc import Callable
from dataclasses import dataclass, field


def get_cpu_model() -> str:
    """Get CPU model name from /proc/cpuinfo or platform."""
    try:
        with open("/proc/cpuinfo") as f:
            for line in f:
                if line.startswith("model name"):
                    return line.split(":", 1)[1].strip()
    except (FileNotFoundError, PermissionError):
        pass
    return platform.processor() or "Unknown"


def get_physical_cores() -> int:
    """Get physical core count."""
    try:
        with open("/proc/cpuinfo") as f:
            physical_ids: set[str] = set()
            cores_per_socket: int = 1
            for line in f:
                if line.startswith("physical id"):
                    physical_ids.add(line.split(":", 1)[1].strip())
                elif line.startswith("cpu cores"):
                    cores_per_socket = int(line.split(":", 1)[1].strip())
            if physical_ids:
                return len(physical_ids) * cores_per_socket
    except (FileNotFoundError, PermissionError):
        pass
    logical = os.cpu_count() or 1
    return max(1, logical // 2)


def get_numba_version() -> str:
    """Get Numba version if available."""
    try:
        import numba  # type: ignore[import-untyped]

        return str(numba.__version__)
    except ImportError:
        return "not installed"


def get_pybevy_version() -> str:
    """Get PyBevy version string."""
    try:
        import pybevy  # type: ignore[import-untyped]

        return str(getattr(pybevy, "__version__", "unknown"))
    except ImportError:
        return "unknown"


def print_system_info() -> None:
    """Print system information header."""
    logical_cores = os.cpu_count() or 0
    physical_cores = get_physical_cores()
    py_version = platform.python_version()
    py_impl = platform.python_implementation()
    os_info = f"{platform.system()} {platform.release()} {platform.machine()}"

    print("=" * 64)
    print("System Information")
    print("=" * 64)
    print(f"  CPU:      {get_cpu_model()}")
    print(f"  Cores:    {physical_cores} physical, {logical_cores} logical")
    print(f"  Python:   {py_version} ({py_impl})")
    print(f"  Numba:    {get_numba_version()}")
    print(f"  OS:       {os_info}")
    print(f"  PyBevy:   {get_pybevy_version()}")
    print("=" * 64)
    print()



@dataclass
class BenchResult:
    """Statistical benchmark result from multiple rounds."""

    median_ms: float
    q1_ms: float
    q3_ms: float
    min_ms: float
    max_ms: float
    std_ms: float
    round_medians: list[float] = field(default_factory=list)

    @property
    def iqr_ms(self) -> float:
        return self.q3_ms - self.q1_ms

    def format(self, unit: str = "ms") -> str:
        """Format as 'median +/- IQR unit'."""
        if unit == "ms" and self.median_ms < 1.0:
            return f"{self.median_ms * 1000:.1f} +/- {self.iqr_ms * 1000:.1f} us"
        return f"{self.median_ms:.3f} +/- {self.iqr_ms:.3f} ms"

    def format_compact(self) -> str:
        """Format compactly for table cells."""
        if self.median_ms < 0.1:
            return f"{self.median_ms * 1000:.1f} +/- {self.iqr_ms * 1000:.1f} us"
        if self.median_ms < 10:
            return f"{self.median_ms:.2f} +/- {self.iqr_ms:.2f} ms"
        return f"{self.median_ms:.1f} +/- {self.iqr_ms:.1f} ms"


def run_benchmark(
    fn: Callable[[], None],
    warmup: int = 5,
    iterations: int = 30,
    rounds: int = 5,
) -> BenchResult:
    """Run a benchmark with statistical treatment.

    For each round:
    1. Run `warmup` iterations (discarded)
    2. Run `iterations` measured iterations
    3. Compute median of measured iterations

    Across rounds: report median, IQR, min, max, std dev of round medians.
    """
    round_medians: list[float] = []

    for _ in range(rounds):
        # Warmup
        for _ in range(warmup):
            fn()

        # Measured iterations
        times: list[float] = []
        for _ in range(iterations):
            t0 = time.perf_counter()
            fn()
            times.append((time.perf_counter() - t0) * 1000)  # ms

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




def print_table_header(
    title: str,
    columns: list[str],
    warmup: int,
    iterations: int,
    rounds: int,
) -> None:
    """Print a formatted table header."""
    print("=" * 80)
    print(f"{title} ({warmup} warmup, {iterations} iter, {rounds} rounds)")
    print("=" * 80)
    header = "  ".join(f"{col:>20}" for col in columns)
    print(header)
    print("-" * 80)


def print_table_footer() -> None:
    """Print table footer."""
    print("=" * 80)
    print()


def compute_speedup(baseline: BenchResult, optimized: BenchResult) -> str:
    """Compute speedup string."""
    if optimized.median_ms <= 0:
        return "inf"
    ratio = baseline.median_ms / optimized.median_ms
    return f"{ratio:.1f}x"




def add_bench_args(parser: argparse.ArgumentParser) -> None:
    """Add standard benchmark CLI arguments."""
    parser.add_argument(
        "--warmup", type=int, default=5, help="Warmup iterations per round (default: 5)"
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=30,
        help="Measured iterations per round (default: 30)",
    )
    parser.add_argument(
        "--rounds", type=int, default=5, help="Number of rounds (default: 5)"
    )


@dataclass
class BenchConfig:
    """Benchmark configuration from CLI args."""

    warmup: int = 5
    iterations: int = 30
    rounds: int = 5

    @classmethod
    def from_args(cls, args: argparse.Namespace) -> BenchConfig:
        return cls(
            warmup=args.warmup,
            iterations=args.iterations,
            rounds=args.rounds,
        )
