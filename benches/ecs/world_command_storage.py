"""Repeated-frame command memory probe for Linux/glibc, emitting JSON.

Run with each isolated wheel's interpreter, from outside the source checkout:
    python /path/to/benches/ecs/world_command_storage.py queued 50000 1000 1000
Use `direct` for the immediate-operation control. Entity storage is allocated
before measurement; every frame replaces components on those same entities.
Timings exclude memory sampling and assertions, but include enqueue and flush.
"""

from __future__ import annotations

import ctypes
import gc
import json
import sys
from dataclasses import asdict, dataclass
from importlib import import_module
from pathlib import Path
from time import perf_counter_ns
from typing import ClassVar, cast

from pybevy.ecs import World
from pybevy.transform import Transform


class Mallinfo2(ctypes.Structure):
    _fields_: ClassVar[list[tuple[str, type[ctypes.c_size_t]]]] = [
        (name, ctypes.c_size_t)
        for name in (
            "arena", "ordblks", "smblks", "hblks", "hblkhd", "usmblks",
            "fsmblks", "uordblks", "fordblks", "keepcost",
        )
    ]


LIBC = ctypes.CDLL("libc.so.6")
LIBC.mallinfo2.argtypes = []
LIBC.mallinfo2.restype = Mallinfo2


@dataclass
class Memory:
    rss: int
    allocated: int


def memory() -> Memory:
    gc.collect()
    rss = next(
        int(line.split()[1]) * 1024
        for line in Path("/proc/self/smaps_rollup").read_text().splitlines()
        if line.startswith("Rss:")
    )
    info = cast(Mallinfo2, LIBC.mallinfo2())
    return Memory(rss, int(info.uordblks) + int(info.hblkhd))


@dataclass
class Frame:
    count: int
    enqueue_ns: int
    flush_ns: int
    pending: Memory
    flushed: Memory


def measure(queued: bool, counts: list[int]) -> dict[str, object]:
    world = World()
    entities = world.spawn_batch(Transform(), count=max(counts))
    source = world.commands() if queued else world
    start = memory()
    frames: list[Frame] = []
    for frame, count in enumerate(counts, start=1):
        value = Transform.from_xyz(float(frame), 0.0, 0.0)
        begin = perf_counter_ns()
        for index in range(count):
            source.entity(entities[index]).insert(value)
        enqueue_ns = perf_counter_ns() - begin
        pending = memory()
        begin = perf_counter_ns()
        world.flush()
        flush_ns = perf_counter_ns() - begin
        flushed = memory()
        if count:
            first = world.get(entities[0], Transform)
            last = world.get(entities[count - 1], Transform)
            assert first is not None
            assert last is not None
            assert first.translation.x == frame
            assert last.translation.x == frame
            del first, last
        frames.append(Frame(count, enqueue_ns, flush_ns, pending, flushed))
    del source, world, entities, value
    return {
        "native": import_module("pybevy._pybevy").__file__,
        "python": sys.version,
        "queued": queued,
        "start": asdict(start),
        "frames": [asdict(frame) for frame in frames],
        "dropped": asdict(memory()),
    }


def main() -> None:
    mode, *sizes = sys.argv[1:]
    if mode not in {"queued", "direct"}:
        raise ValueError("mode must be queued or direct")
    counts = [int(size) for size in sizes]
    if not counts or min(counts) < 0 or max(counts) == 0:
        raise ValueError("provide nonnegative frame counts with at least one nonzero")
    print(json.dumps(measure(mode == "queued", counts)))


if __name__ == "__main__":
    main()
