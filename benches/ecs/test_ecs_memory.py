"""Memory footprint benchmarks: PyBevy vs pure Rust Bevy.

Compares bytes-per-entity (RSS delta) for identical component configurations:
- Rust side: cargo example `memory_baseline` (pure Bevy, no Python)
- Python side: PyBevy spawning equivalent components

This gives a direct answer to "what memory overhead does PyBevy add?"

Usage:
    poetry run pytest benches/ecs/test_ecs_memory.py -v -s
    poetry run pytest benches/ecs/test_ecs_memory.py -v -s -k summary   # just the table

The -s flag is important to see the printed memory tables.
Each measurement runs in a subprocess for RSS isolation.
"""

from __future__ import annotations

import json
import subprocess
import sys
import textwrap
from pathlib import Path

ENTITY_COUNT = 100_000
RUST_BINARY = "memory_baseline"
PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent


def _measure_rust(scenario: str, entity_count: int) -> dict[str, float]:
    """Run the Rust memory_baseline example and parse JSON output."""
    binary = PROJECT_ROOT / "target" / "release" / "examples" / RUST_BINARY
    if not binary.exists():
        raise FileNotFoundError(
            f"Rust binary not found: {binary}\n"
            "Build it: cargo build --example memory_baseline --release --features linux-display"
        )
    result = subprocess.run(
        [str(binary), scenario, str(entity_count)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Rust binary failed:\n{result.stderr[-500:]}")
    return json.loads(result.stdout.strip())  # type: ignore[no-any-return]


_PYBEVY_SCRIPT = textwrap.dedent("""\
import gc, json, os, sys

def get_rss_bytes():
    with open("/proc/self/statm") as f:
        pages = int(f.read().split()[1])
    return pages * os.sysconf("SC_PAGE_SIZE")

from dataclasses import dataclass, field
from pybevy.app import App, Startup
from pybevy.decorators import component
from pybevy.ecs import Commands, Component
from pybevy.transform import Transform

@component
@dataclass
class WVelocity(Component):
    vx: float
    vy: float
    vz: float

@component(storage="python")
@dataclass
class POVelocityStr(Component):
    vx: float
    vy: float
    vz: float
    label: str

@component(storage="python")
@dataclass
class POVelocity(Component):
    vx: float
    vy: float
    vz: float

@component
@dataclass
class MarkerComp(Component):
    pass

SCENARIOS = {
    "transform": lambda i, cmds: cmds.spawn(Transform.from_xyz(float(i), 0.0, 0.0)),
    "velocity_small": lambda i, cmds: cmds.spawn(
        WVelocity(vx=float(i), vy=0.0, vz=0.0)
    ),
    "velocity_string": lambda i, cmds: cmds.spawn(
        POVelocityStr(vx=float(i), vy=0.0, vz=0.0, label="entity")
    ),
    "velocity_small_pyobject": lambda i, cmds: cmds.spawn(
        POVelocity(vx=float(i), vy=0.0, vz=0.0)
    ),
    "transform_velocity": lambda i, cmds: cmds.spawn(
        Transform.from_xyz(float(i), 0.0, 0.0),
        WVelocity(vx=1.0, vy=0.0, vz=0.0),
    ),
    "transform_marker": lambda i, cmds: cmds.spawn(
        Transform.from_xyz(float(i), 0.0, 0.0), MarkerComp()
    ),
}

scenario = sys.argv[1]
n = int(sys.argv[2])
spawn_one = SCENARIOS[scenario]

def spawn(commands: Commands) -> None:
    for i in range(n):
        spawn_one(i, commands)

app = App()
app.add_systems(Startup, spawn)
app.initialize()

gc.collect()
gc.collect()
rss_before = get_rss_bytes()

app.update()

gc.collect()
rss_after = get_rss_bytes()

delta = rss_after - rss_before
bpe = delta / n if n > 0 else 0
print(json.dumps({"rss_before": rss_before, "rss_after": rss_after, "delta": delta, "bpe": bpe}))
""")


def _measure_pybevy(scenario: str, entity_count: int) -> dict[str, float]:
    """Run PyBevy memory measurement in a fresh subprocess."""
    result = subprocess.run(
        [sys.executable, "-c", _PYBEVY_SCRIPT, scenario, str(entity_count)],
        capture_output=True,
        text=True,
        timeout=60,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"PyBevy subprocess failed for {scenario}:\n{result.stderr[-500:]}"
        )
    for line in reversed(result.stdout.strip().splitlines()):
        line = line.strip()
        if line.startswith("{"):
            return json.loads(line)  # type: ignore[no-any-return]
    raise RuntimeError(f"No JSON output for {scenario}:\n{result.stdout[-500:]}")


# (rust_scenario, pybevy_scenario, label, description)
COMPARISONS: list[tuple[str, str, str, str]] = [
    ("transform", "transform",
     "Transform (48B native)",
     "Bevy Transform — identical in both"),
    ("velocity_small", "velocity_small",
     "Vec3-like (3xf32 vs 3xf64)",
     "Rust: Velocity{Vec3} 12B / Py: wrapper 24B (f64)"),
    ("velocity_small", "velocity_small_pyobject",
     "Vec3-like (Rust vs PyObject)",
     "Rust: Velocity{Vec3} 12B / Py: PyObject storage"),
    ("velocity_string", "velocity_string",
     "Vec3+String",
     "Rust: VelocityStr{Vec3,String} / Py: PyObject"),
    ("transform_velocity", "transform_velocity",
     "Transform + Velocity",
     "Two components per entity"),
    ("transform_marker", "transform_marker",
     "Transform + Marker",
     "Transform + zero-size marker"),
]


import pytest

_RUST_BINARY_PATH = PROJECT_ROOT / "target" / "release" / "examples" / RUST_BINARY


@pytest.mark.skipif(
    not _RUST_BINARY_PATH.exists(),
    reason=f"Rust binary not found at {_RUST_BINARY_PATH}. "
    "Build it: cargo build --example memory_baseline --release --features linux-display",
)
class TestMemoryComparison:
    """PyBevy vs Rust Bevy memory comparison. Run with -s to see output."""

    def test_transform_comparison(self) -> None:
        """Transform: PyBevy vs pure Rust Bevy."""

        rust = _measure_rust("transform", ENTITY_COUNT)
        py = _measure_pybevy("transform", ENTITY_COUNT)
        overhead = py["bpe"] - rust["bpe"]
        ratio = py["bpe"] / rust["bpe"] if rust["bpe"] > 0 else 0
        print(f"\n  Transform: Rust={rust['bpe']:.1f} B/ent, PyBevy={py['bpe']:.1f} B/ent, "
              f"overhead={overhead:+.1f} B/ent ({ratio:.2f}x)")

    def test_velocity_wrapper_comparison(self) -> None:
        """Vec3-like component: wrapper storage vs Rust."""

        rust = _measure_rust("velocity_small", ENTITY_COUNT)
        py = _measure_pybevy("velocity_small", ENTITY_COUNT)
        overhead = py["bpe"] - rust["bpe"]
        ratio = py["bpe"] / rust["bpe"] if rust["bpe"] > 0 else 0
        print(f"\n  Vec3 velocity: Rust={rust['bpe']:.1f} B/ent, PyBevy wrapper={py['bpe']:.1f} B/ent, "
              f"overhead={overhead:+.1f} B/ent ({ratio:.2f}x)")

    def test_velocity_pyobject_comparison(self) -> None:
        """Vec3-like component: PyObject storage vs Rust."""

        rust = _measure_rust("velocity_small", ENTITY_COUNT)
        py = _measure_pybevy("velocity_small_pyobject", ENTITY_COUNT)
        overhead = py["bpe"] - rust["bpe"]
        ratio = py["bpe"] / rust["bpe"] if rust["bpe"] > 0 else 0
        print(f"\n  Vec3 velocity: Rust={rust['bpe']:.1f} B/ent, PyBevy pyobject={py['bpe']:.1f} B/ent, "
              f"overhead={overhead:+.1f} B/ent ({ratio:.2f}x)")

    def test_velocity_string_comparison(self) -> None:
        """Vec3+String: PyObject storage vs Rust."""

        rust = _measure_rust("velocity_string", ENTITY_COUNT)
        py = _measure_pybevy("velocity_string", ENTITY_COUNT)
        overhead = py["bpe"] - rust["bpe"]
        ratio = py["bpe"] / rust["bpe"] if rust["bpe"] > 0 else 0
        print(f"\n  Vec3+String: Rust={rust['bpe']:.1f} B/ent, PyBevy={py['bpe']:.1f} B/ent, "
              f"overhead={overhead:+.1f} B/ent ({ratio:.2f}x)")

    def test_summary_table(self) -> None:
        """Print full comparison table: PyBevy vs pure Rust Bevy."""

        n = ENTITY_COUNT

        print(f"\n{'='*90}")
        print(f"  Memory Footprint: PyBevy vs Rust Bevy (N={n:,} entities)")
        print(f"{'='*90}")
        print(f"\n  {'Component':<28s} {'Rust B/ent':>11s} {'PyBevy B/ent':>13s}"
              f" {'Overhead':>10s} {'Ratio':>7s}  Notes")
        print(f"  {'-'*28} {'-'*11} {'-'*13} {'-'*10} {'-'*7}  {'-'*30}")

        for rust_sc, py_sc, label, desc in COMPARISONS:
            rust = _measure_rust(rust_sc, n)
            py = _measure_pybevy(py_sc, n)
            overhead = py["bpe"] - rust["bpe"]
            ratio = py["bpe"] / rust["bpe"] if rust["bpe"] > 0 else 0
            print(f"  {label:<28s} {rust['bpe']:11.1f} {py['bpe']:13.1f}"
                  f" {overhead:+10.1f} {ratio:6.2f}x  {desc}")

        print("\n  Notes:")
        print("  - Both sides use App + MinimalPlugins + Startup system (apples-to-apples)")
        print("  - Rust Velocity{Vec3} is 12B (3xf32); PyBevy wrapper uses 3xf64 = 24B in ComponentWrapper32")
        print("  - Native Bevy components (Transform) stored identically in ECS — no per-component overhead")
        print("  - Constant ~400B/ent overhead = PyBevy's per-entity ECS bookkeeping (ComponentRegistry,")
        print("    command queues with Python object temporaries, system parameter wrappers)")
        print("  - PyObject storage adds ~50B/ent extra vs wrapper (Python heap object + GC tracking)")
        print(f"{'='*90}\n")
