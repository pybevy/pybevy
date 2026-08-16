"""Engine process management: display detection and subprocess env building."""

from __future__ import annotations

import os
import subprocess
import sys
import threading


def _is_linux(platform: str | None = None) -> bool:
    """Return whether ``platform`` names Linux, defaulting to this host."""
    return (sys.platform if platform is None else platform).startswith("linux")


def _read_env_from_session(var: str, *, platform: str | None = None) -> str | None:
    """Try to read a Linux display variable from the user's login session."""
    if not _is_linux(platform):
        return None

    try:
        result = subprocess.run(
            ["systemctl", "--user", "show-environment"],
            capture_output=True,
            text=True,
            timeout=2,
        )
        if result.returncode == 0:
            for line in result.stdout.splitlines():
                if line.startswith(f"{var}="):
                    return line.split("=", 1)[1]
    except (FileNotFoundError, subprocess.TimeoutExpired):
        pass

    uid = os.getuid()
    defaults: dict[str, str] = {
        "DISPLAY": ":0",
        "WAYLAND_DISPLAY": "wayland-0",
        "XDG_RUNTIME_DIR": f"/run/user/{uid}",
    }
    if var in defaults:
        candidate = defaults[var]
        if var == "XDG_RUNTIME_DIR":
            if os.path.isdir(candidate):
                return candidate
        elif var == "WAYLAND_DISPLAY":
            runtime_dir = os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{uid}")
            if os.path.exists(os.path.join(runtime_dir, candidate)):
                return candidate
        elif var == "DISPLAY" and os.path.exists(
            f"/tmp/.X11-unix/X{candidate.lstrip(':')}"
        ):
            return candidate

    return None


def build_engine_env(
    port: int | None = None, *, platform: str | None = None
) -> dict[str, str]:
    """Build environment for the engine subprocess with display vars and MCP injection.

    Args:
        port: Control server port. If set, passed via PYBEVY_CONTROL_PORT env var.
    """
    env = os.environ.copy()
    env["PYTHONUNBUFFERED"] = "1"
    env["PYBEVY_MCP"] = "1"
    env["NO_COLOR"] = "1"

    if port is not None:
        env["PYBEVY_CONTROL_PORT"] = str(port)

    if _is_linux(platform):
        for var in (
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "XDG_RUNTIME_DIR",
            "XDG_SESSION_TYPE",
            "DBUS_SESSION_BUS_ADDRESS",
        ):
            if var not in env:
                val = _read_env_from_session(var, platform=platform)
                if val:
                    env[var] = val

    return env


DEFAULT_CONTROL_PORT_RANGE = (8420, 8499)
CONTROL_PORT_RANGE_ENV = "PYBEVY_CONTROL_PORT_RANGE"


def control_port_range() -> tuple[int, int]:
    """The inclusive port range engines may bind, as "START-END".

    Probing a port frees it again before the engine binds it, so two bridges
    scanning the same range can be handed the same number and the second engine
    fails to start. Give each one its own range instead.
    """
    raw = os.environ.get(CONTROL_PORT_RANGE_ENV, "").strip()
    if not raw:
        return DEFAULT_CONTROL_PORT_RANGE

    start_text, separator, end_text = raw.partition("-")
    try:
        if not separator:
            raise ValueError
        start, end = int(start_text), int(end_text)
    except ValueError:
        msg = f"{CONTROL_PORT_RANGE_ENV} must look like '8420-8499', got {raw!r}"
        raise ValueError(msg) from None
    if not 0 < start <= end <= 65535:
        msg = (
            f"{CONTROL_PORT_RANGE_ENV} range is out of order or out of bounds: {raw!r}"
        )
        raise ValueError(msg)
    return start, end


_handed_out: set[int] = set()
_handed_out_lock = threading.Lock()


def _bindable(port: int) -> bool:
    import socket  # noqa: PLC0415

    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.bind(("127.0.0.1", port))
    except OSError:
        return False
    return True


def find_free_port(start: int | None = None, end: int | None = None) -> int:
    """Find a free TCP port, defaulting to the configured control range.

    Probing frees the port again, so an engine that is still starting up has
    not bound its port yet and the next probe would hand out the same number.
    Ports already returned in this process are skipped until the range is
    exhausted, by which point the engines holding them are long gone.
    """
    range_start, range_end = control_port_range()
    start = range_start if start is None else start
    end = range_end if end is None else end
    ports = range(start, end + 1)

    with _handed_out_lock:
        for attempt in (ports, ports):
            for port in attempt:
                if port in _handed_out:
                    continue
                if _bindable(port):
                    _handed_out.add(port)
                    return port
            # Nothing unused left: forget the history and allow reuse.
            _handed_out.difference_update(ports)

    msg = f"No free ports in range {start}-{end}"
    raise RuntimeError(msg)
