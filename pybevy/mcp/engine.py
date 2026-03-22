"""Engine process management — display detection and subprocess env building."""

from __future__ import annotations

import os
import subprocess


def has_display() -> bool:
    """Check whether a usable display (X11 or Wayland) is available."""
    display = os.environ.get("DISPLAY", "")
    wayland = os.environ.get("WAYLAND_DISPLAY", "")

    if display:
        display_num = display.lstrip(":").split(".")[0]
        if os.path.exists(f"/tmp/.X11-unix/X{display_num}"):
            return True

    if wayland:
        uid = os.getuid()
        runtime_dir = os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{uid}")
        if os.path.exists(os.path.join(runtime_dir, wayland)):
            return True

    for var in ("DISPLAY", "WAYLAND_DISPLAY"):
        val = _read_env_from_session(var)
        if val:
            return True

    return False


def _read_env_from_session(var: str) -> str | None:
    """Try to read a display env var from the user's login session."""
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
        elif var == "DISPLAY" and os.path.exists(f"/tmp/.X11-unix/X{candidate.lstrip(':')}"):
            return candidate

    return None


def build_engine_env(port: int | None = None) -> dict[str, str]:
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

    for var in (
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "XDG_SESSION_TYPE",
        "DBUS_SESSION_BUS_ADDRESS",
    ):
        if var not in env:
            val = _read_env_from_session(var)
            if val:
                env[var] = val

    return env


def find_free_port(start: int = 8420, end: int = 8499) -> int:
    """Find a free TCP port in the given range."""
    import socket  # noqa: PLC0415

    for port in range(start, end + 1):
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                s.bind(("127.0.0.1", port))
                return port
        except OSError:
            continue
    msg = f"No free ports in range {start}-{end}"
    raise RuntimeError(msg)
