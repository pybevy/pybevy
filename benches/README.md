# Benchmarks

```bash
poetry run maturin develop --release
```

- `ecs/`, `image/`, `math/`, `mesh/` - pytest-benchmark suites (`poetry run pytest benches/ --benchmark-only`)
- `paper/` - Reproducible benchmarks for the architecture paper (see [paper/README.md](paper/README.md))

## World command storage

`ecs/world_command_storage.py` measures repeated inserts on one World, with
entity storage allocated before sampling. It reports enqueue/flush timings,
RSS, and glibc allocator-accounted live bytes. RSS includes allocator retention;
it is not a count of live command payloads. Run on Linux/glibc with separate
immutable release-wheel environments, from outside either source checkout:

```bash
<baseline-python> /path/to/ecs/world_command_storage.py queued 50000 1000 1000 0
<candidate-python> /path/to/ecs/world_command_storage.py queued 50000 1000 1000 0
```

Repeat in alternating process order. Use `direct` for the immediate-operation
control. For throughput, supply repeated equal-size frames, discard initial
warmups, and compare enqueue plus flush time. Memory sampling and assertions
are outside timing. The JSON records the loaded extension path; verify it.
These timings reuse a constructed Transform and do not measure its constructor,
batch spawning, observers, or rendering. Do not run other benchmarks concurrently.

`ecs/world_command_queue.rs` isolates Bevy's queue allocation with the same
two-weak-reference-plus-ID entry shape, but no Python payload storage. After
the release build, compile it against that target's dependencies:

```bash
rustc --edition=2024 -O benches/ecs/world_command_queue.rs \
  -L dependency=target/release/deps \
  --extern bevy_ecs=$(rg --files target/release/deps -g 'libbevy_ecs-*.rlib') \
  --extern libc=$(rg --files target/release/deps -g 'liblibc-*.rlib') \
  -o target/world-command-queue
target/world-command-queue 50000 1000 1000 0
```

Each dependency lookup must identify exactly one matching release artifact.
Use an isolated target per worktree. Reuse within the same worktree is safe
after preserving the baseline wheel; sharing targets between worktrees is not.

For runtime verification, install dependencies into the selected environment
instead of borrowing another environment's site-packages through `PYTHONPATH`.
Keep environments and downloaded interpreters outside the source tree or under
`target/` or `.venv/`: the reload import graph scans ordinary `.cache/` directories.
Watcher-test scene directories must instead stay outside ignored directories.
Use separate verification worktrees for normal and free-threaded CPython:
the fresh-process restore fixture expects exactly one native extension file.
