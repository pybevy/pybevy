#!/usr/bin/env python3
"""Paper benchmark: Rendering throughput vs entity count.

Spawns increasing numbers of rendered cubes (mesh + material), animates them
with a sine wave pattern via View + Numba, and measures frame time to establish
the practical rendering throughput limit.

Uses a single App run: spawns the first batch, measures, then spawns more
entities to reach the next count, measures again, etc. This avoids winit's
one-event-loop-per-process limitation.

Usage:
    poetry run python benches/paper/rendering_fps.py
    poetry run python benches/paper/rendering_fps.py --counts 10000 50000 100000
    poetry run python benches/paper/rendering_fps.py --frames 30  # quick test

Requirements:
    - PyBevy built in release mode: poetry run maturin develop --release
    - Display/GPU available (uses DefaultPlugins with rendering)
    - Numba installed: poetry install --extras numba
"""

from __future__ import annotations

import argparse
import math
import statistics

import numba  # type: ignore[import-untyped]

from benches.paper.bench_utils import print_system_info
from pybevy.app import App, AppExit, DefaultPlugins, Startup, Update
from pybevy.assets import Assets, Handle
from pybevy.camera import Camera3d
from pybevy.ecs import Commands, MessageWriter, Mut, Res, ResMut, View, With
from pybevy.math import Cuboid, Vec3
from pybevy.mesh import Mesh, Mesh3d, MeshMaterial3d
from pybevy.render import StandardMaterial
from pybevy.time import Time
from pybevy.transform import Transform
from pybevy.window import PresentMode, Window, WindowPlugin


DEFAULT_COUNTS = [
    1_000, 5_000, 10_000, 50_000,
    80_000, 90_000, 100_000, 110_000, 120_000, 130_000, 140_000, 150_000,
    200_000, 250_000, 400_000,
]
DEFAULT_WARMUP_FRAMES = 60
DEFAULT_MEASURE_FRAMES = 120


@numba.jit(nopython=True, parallel=True)
def _wave_kernel(
    translation,
    time: float,
) -> None:
    """Sine wave on Y based on XZ distance from origin."""
    n = len(translation.x)
    for i in numba.prange(n):
        x = translation.x[i]
        z = translation.z[i]
        dist = math.sqrt(x * x + z * z)
        translation.y[i] = math.sin(dist * 0.5 - time * 2.0) * 3.0


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Rendering throughput benchmark: FPS vs entity count"
    )
    parser.add_argument(
        "--counts",
        type=int,
        nargs="+",
        default=DEFAULT_COUNTS,
        help=f"Entity counts to test (default: {DEFAULT_COUNTS})",
    )
    parser.add_argument(
        "--warmup-frames",
        type=int,
        default=DEFAULT_WARMUP_FRAMES,
        help=f"Warmup frames before measurement (default: {DEFAULT_WARMUP_FRAMES})",
    )
    parser.add_argument(
        "--frames",
        type=int,
        default=DEFAULT_MEASURE_FRAMES,
        help=f"Measured frames per entity count (default: {DEFAULT_MEASURE_FRAMES})",
    )
    args = parser.parse_args()

    counts: list[int] = sorted(args.counts)
    warmup_frames: int = args.warmup_frames
    measure_frames: int = args.frames

    print_system_info()

    header = (
        f"Rendering FPS Benchmark "
        f"({warmup_frames} warmup, {measure_frames} measured frames)"
    )
    print(header)
    print("=" * len(header))
    print()
    print(
        f"{'Entities':>10}  {'Median FPS':>12}  {'Min FPS':>10}  {'Max FPS':>10}"
        f"  {'Frame ms':>18}  {'Status':>8}"
    )
    print("-" * 82)

    # State shared between setup and the per-frame system
    mesh_handle: list[Handle] = []
    material_handle: list[Handle] = []
    current_entities = [0]
    phase_idx = [0]          # which count we're working on
    frame_in_phase = [0]     # frame counter within current phase
    phase_frame_times: list[float] = []
    results: list[dict[str, float]] = []

    def setup(
        commands: Commands,
        meshes: ResMut[Assets[Mesh]],
        materials: ResMut[Assets[StandardMaterial]],
    ) -> None:
        # Camera must see entire grid. With default 45° FOV and spacing=1.2:
        #   half_extent = (sqrt(max_count) + 1) * 1.2 / 2
        #   distance = half_extent / tan(22.5°) ≈ half_extent * 2.5
        max_side = int(counts[-1] ** 0.5) + 1
        half_extent = max_side * 1.2 / 2.0
        cam_dist = half_extent * 2.5
        commands.spawn(
            Camera3d(),
            Transform.from_xyz(0.0, cam_dist * 0.7, cam_dist).looking_at(
                Vec3.ZERO, Vec3.Y
            ),
        )
        mesh_handle.append(meshes.add(Cuboid(0.5, 0.5, 0.5).mesh()))
        material_handle.append(materials.add(StandardMaterial()))

    def _spawn_batch(commands: Commands, count: int) -> None:
        """Spawn `count` more cubes, continuing the grid from current_entities."""
        start = current_entities[0]
        total = start + count
        side = int(total**0.5) + 1
        spacing = 1.2
        mh = mesh_handle[0]
        mat = material_handle[0]
        for i in range(start, total):
            x = (i % side) * spacing - (side * spacing / 2.0)
            z = (i // side) * spacing - (side * spacing / 2.0)
            commands.spawn(
                Mesh3d(mh),
                MeshMaterial3d(mat),
                Transform.from_xyz(x, 0.0, z),
            )
        current_entities[0] = total

    def animate(
        view: View[Mut[Transform], With[Mesh3d]],
        time_res: Res[Time],
    ) -> None:
        t = time_res.elapsed_secs()
        for batch in view.iter_batches():
            col = batch.column_mut(Transform)
            _wave_kernel(col.translation, t)

    def tick(
        commands: Commands,
        time_res: Res[Time],
        exit_writer: MessageWriter[AppExit],
    ) -> None:
        if phase_idx[0] >= len(counts):
            return

        target = counts[phase_idx[0]]

        # Spawn more entities if needed (first frame of a new phase)
        if current_entities[0] < target:
            _spawn_batch(commands, target - current_entities[0])
            # Reset phase counters; give one frame for the spawn to apply
            frame_in_phase[0] = 0
            phase_frame_times.clear()
            return

        frame_in_phase[0] += 1

        # Warmup phase
        if frame_in_phase[0] <= warmup_frames:
            return

        # Measurement phase
        dt = time_res.delta_secs()
        if dt > 0:
            phase_frame_times.append(dt)

        if frame_in_phase[0] >= warmup_frames + measure_frames:
            # Collect results for this count
            if phase_frame_times:
                frame_ms = sorted(t * 1000.0 for t in phase_frame_times)
                fps_values = [1.0 / t for t in phase_frame_times if t > 0]
                q1 = frame_ms[len(frame_ms) // 4]
                q3 = frame_ms[3 * len(frame_ms) // 4]
                results.append({
                    "count": target,
                    "median_fps": statistics.median(fps_values),
                    "min_fps": min(fps_values),
                    "max_fps": max(fps_values),
                    "median_frame_ms": statistics.median(frame_ms),
                    "iqr_frame_ms": q3 - q1,
                })
            else:
                results.append({
                    "count": target,
                    "median_fps": 0.0,
                    "min_fps": 0.0,
                    "max_fps": 0.0,
                    "median_frame_ms": 0.0,
                    "iqr_frame_ms": 0.0,
                })

            # Print row immediately
            r = results[-1]
            fps = r["median_fps"]
            status = "OK" if fps >= 60 else ("WARN" if fps >= 30 else "SLOW")
            frame_str = (
                f"{r['median_frame_ms']:.2f} +/- {r['iqr_frame_ms']:.2f} ms"
            )
            print(
                f"{target:>10,}  {fps:>12.1f}  {r['min_fps']:>10.1f}"
                f"  {r['max_fps']:>10.1f}  {frame_str:>18}  {status:>8}"
            )

            # Move to next phase
            phase_idx[0] += 1
            frame_in_phase[0] = 0
            phase_frame_times.clear()

            if phase_idx[0] >= len(counts):
                exit_writer.write(AppExit.SUCCESS)

    window = Window()
    window.present_mode = PresentMode.AutoNoVsync
    app = App()
    app.add_plugins(DefaultPlugins().set(WindowPlugin(primary_window=window)))
    app.add_systems(Startup, setup)
    app.add_systems(Update, (animate, tick))
    app.run()

    print()
    print("Status: OK = >=60 FPS, WARN = 30-60 FPS, SLOW = <30 FPS")


if __name__ == "__main__":
    main()
