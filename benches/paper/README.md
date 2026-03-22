# Paper Benchmark Suite

Reproducible benchmarks for the PyBevy architecture paper. Each script produces
formatted tables with system information and statistical treatment (median +/- IQR
across multiple rounds).

## Prerequisites

```bash
# Build PyBevy in release mode (required for accurate results)
poetry run maturin develop --release

# Install Numba (required for scaling/physics/native/rendering benchmarks)
poetry install -E numba
```

## Scripts

| Script | Paper Table | Description |
|--------|------------|-------------|
| `scaling.py` | Table 2 | Query vs View API vs Numba scaling (1K-10M entities) |
| `native_comparison.py` | Table 3 | All tiers vs native Bevy at 1M entities |
| `physics.py` | Table 4 | Physics workload benchmark (5K entities) |
| `dispatch_overhead.py` | Table 5 | View API workload profiles at 1M entities |
| `rendering_fps.py` | Table 6 | Rendering FPS vs entity count — PyBevy (requires display/GPU) |
| `rendering_fps_native.rs` | Table 6 | Rendering FPS vs entity count — native Bevy (requires display/GPU) |
| `flocking.py` | Table 7 | O(n²) boids flocking — Query vs Numba (1T + parallel) |
| `numpy_comparison.py` | §5.3 | NumPy flat-array baseline (increment + physics) |

## Quick Smoke Test

```bash
poetry run python benches/paper/scaling.py --rounds 1 --iterations 3 --counts 1000
poetry run python benches/paper/physics.py --rounds 1 --iterations 3 --entities 1000
poetry run python benches/paper/native_comparison.py --rounds 1 --iterations 3 --entities 1000
poetry run python benches/paper/dispatch_overhead.py --rounds 1 --iterations 3 --entities 1000
poetry run python benches/paper/flocking.py --rounds 1 --iterations 3 --counts 500
poetry run python benches/paper/numpy_comparison.py --rounds 1 --iterations 3
```

## Full Benchmark Run

```bash
# 1. Native Rust baselines (run first — values used by native_comparison.py)
cargo bench --bench vm_overhead        # increment x += 1.0 baseline
cargo bench --bench native_physics     # physics kernel baseline (trig)
cargo bench --bench query_dispatch     # static vs dynamic dispatch
cargo bench --bench validity_overhead  # validity flag AtomicU8 overhead

# 2. Python benchmarks (defaults: 5 warmup, 30 measured iterations, 5 rounds)
poetry run python benches/paper/scaling.py
poetry run python benches/paper/physics.py
poetry run python benches/paper/native_comparison.py
poetry run python benches/paper/dispatch_overhead.py
poetry run python benches/paper/flocking.py
poetry run python benches/paper/numpy_comparison.py

# 3. Rendering benchmarks (require display/GPU, disable vsync)
#    PyBevy (View + Numba sine wave animation)
poetry run python benches/paper/rendering_fps.py
#    Native Bevy (Rust par_iter_mut sine wave animation)
cargo run --example rendering_fps_native --release --features linux-display
```

## CLI Options

All scripts share these options via `bench_utils`:

- `--warmup N` — Warmup iterations per round (default: 5)
- `--iterations N` — Measured iterations per round (default: 30)
- `--rounds N` — Number of rounds (default: 5)

Script-specific options:

- `scaling.py --counts 1000 10000 100000` — Entity counts to benchmark
- `scaling.py --skip-query-above 100000` — Skip slow Query above threshold
- `physics.py --entities 5000` — Entity count for physics workload
- `native_comparison.py --native-ms 0.513` — Native Bevy increment baseline (ms)
- `native_comparison.py --native-physics-ms 0.672` — Native Bevy physics baseline (ms)
- `native_comparison.py --skip-query` — Skip Query (very slow at 1M)
- `physics.py --native-physics-ms 0.672` — Show native Rust baseline row (optional)
- `rendering_fps.py --counts 1000 10000 50000` — Entity counts to test
- `rendering_fps.py --warmup-frames 60` — Warmup frames before measurement
- `rendering_fps.py --frames 120` — Measured frames per entity count
- `flocking.py --counts 500 1000 2000 5000` — Entity counts for flocking
- `numpy_comparison.py --elements 1000000` — Number of float32 elements

## Output Format

Each script prints:
1. System information header (CPU, cores, Python/Numba versions, OS)
2. Benchmark configuration (warmup, iterations, rounds)
3. Results table with median +/- IQR

## Shared Infrastructure

`bench_utils.py` provides:
- `print_system_info()` — System identification
- `run_benchmark()` — Statistical framework
- `BenchResult` — Result dataclass with formatting
- `BenchConfig` — CLI argument handling
- `add_bench_args()` — Standard CLI arguments
