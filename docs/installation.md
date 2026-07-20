# Installation

```bash
pip install pybevy --upgrade
```

PyBevy requires Python 3.12 or newer.

## Pre-compiled Wheels

Wheels are published for the following platforms:

| Platform | Architecture                |
| -------- | --------------------------- |
| Linux    | x86_64                      |
| macOS    | ARM (Apple Silicon), x86_64 |
| Windows  | x86_64                      |

On any other platform, pip falls back to building from source (see below).

## Linux System Dependencies

PyBevy's pre-built wheels link against system display and audio libraries.
On most desktop distributions these are already present.

If you see an `ImportError` mentioning `libwayland-client.so` or `libasound.so`:

```bash
# Debian/Ubuntu
sudo apt install libwayland-client0 libasound2t64

# Fedora/RHEL
sudo dnf install alsa-lib wayland
```

ALSA warnings about missing audio devices are harmless and can be ignored.

### Docker / Headless Environments

Headless environments additionally need a software GPU driver:

```bash
apt install -y libwayland-client0 libasound2t64 mesa-vulkan-drivers
```

## Building From Source

Building from source requires a Rust toolchain plus development headers:

```bash
# Debian/Ubuntu
sudo apt install libwayland-dev libasound2-dev

# Fedora/RHEL
sudo dnf install wayland-devel alsa-lib-devel
```

Then:

```bash
pip install pybevy --no-binary pybevy
```

## Free-Threaded Python (3.13t+)

PyBevy supports Python's free-threaded mode (PEP 703). Non-conflicting Python
systems run truly in parallel on separate cores via Bevy's multi-threaded
scheduler, with no GIL serialization. Validated on CPython 3.14t.

Performance depends on workload and scene complexity.

## Verifying the Install

```bash
python -c "import pybevy; print('ok')"
```

If that succeeds but a script fails to open a window, the problem is almost
always a missing system library or GPU driver rather than PyBevy itself. Run
with `RUST_LOG=debug` for details.
