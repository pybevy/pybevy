"""Session manager for the PyBevy Hub.

Manages PyBevy engine subprocesses on behalf of remote MCP bridges.
Each session corresponds to one running scene with its own HTTP control port.
"""

from __future__ import annotations

import contextlib
import logging
import subprocess
import sys
import threading
import time
import uuid
from dataclasses import dataclass, field
from typing import Any

from ..engine import build_engine_env

logger = logging.getLogger("pybevy.hub")

type JsonDict = dict[str, Any]

_PORT_RANGE_START = 8420
_PORT_RANGE_END = 8499


@dataclass
class Session:
    """A managed PyBevy engine subprocess."""

    session_id: str
    project_dir: str
    scene_path: str
    port: int
    status: str = "starting"  # starting | running | crashed | stopped
    created_at: float = field(default_factory=time.time)
    last_activity: float = field(default_factory=time.time)
    restart_count: int = 0
    max_restarts: int = 5
    process: subprocess.Popen[bytes] | None = field(default=None, repr=False)

    def to_dict(self) -> JsonDict:
        return {
            "session_id": self.session_id,
            "project_dir": self.project_dir,
            "scene_path": self.scene_path,
            "port": self.port,
            "status": self.status,
            "created_at": self.created_at,
            "last_activity": self.last_activity,
            "restart_count": self.restart_count,
            "max_restarts": self.max_restarts,
        }


class SessionManager:
    """Thread-safe manager for PyBevy engine sessions."""

    def __init__(self) -> None:
        self._sessions: dict[str, Session] = {}
        self._lock = threading.Lock()
        self._monitor_thread: threading.Thread | None = None
        self._monitor_running = False
        self._monitor_stop = threading.Event()

    def create_session(
        self,
        project_dir: str,
        scene_path: str,
        session_id: str | None = None,
    ) -> Session:
        """Create and start a new engine session."""
        sid = session_id or uuid.uuid4().hex[:12]
        port = self._allocate_port()

        session = Session(
            session_id=sid,
            project_dir=project_dir,
            scene_path=scene_path,
            port=port,
        )

        self._spawn_process(session)

        with self._lock:
            self._sessions[sid] = session

        logger.info("Created session %s on port %d for %s", sid, port, scene_path)
        return session

    def get_session(self, session_id: str) -> Session | None:
        with self._lock:
            return self._sessions.get(session_id)

    def list_sessions(self) -> list[Session]:
        with self._lock:
            return list(self._sessions.values())

    def restart_session(self, session_id: str) -> Session | None:
        with self._lock:
            session = self._sessions.get(session_id)
            if session is None:
                return None

        self._kill_process(session)
        session.restart_count += 1
        session.status = "starting"
        self._spawn_process(session)
        session.last_activity = time.time()
        logger.info("Restarted session %s (restart #%d)", session_id, session.restart_count)
        return session

    def destroy_session(self, session_id: str) -> bool:
        with self._lock:
            session = self._sessions.pop(session_id, None)
        if session is None:
            return False

        self._kill_process(session)
        session.status = "stopped"
        logger.info("Destroyed session %s", session_id)
        return True

    def shutdown_all(self) -> None:
        self.stop_monitor()
        with self._lock:
            sessions = list(self._sessions.values())
            self._sessions.clear()

        for session in sessions:
            self._kill_process(session)
            session.status = "stopped"

        logger.info("Shut down %d session(s)", len(sessions))

    def _allocate_port(self) -> int:
        import socket  # noqa: PLC0415

        with self._lock:
            used_ports = {s.port for s in self._sessions.values()}
        for port in range(_PORT_RANGE_START, _PORT_RANGE_END + 1):
            if port in used_ports:
                continue
            try:
                with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                    s.bind(("127.0.0.1", port))
                    return port
            except OSError:
                continue
        msg = f"No free ports in range {_PORT_RANGE_START}-{_PORT_RANGE_END}"
        raise RuntimeError(msg)

    def _spawn_process(self, session: Session) -> None:
        env = build_engine_env()
        env["PYBEVY_CONTROL_PORT"] = str(session.port)

        session.process = subprocess.Popen(
            [sys.executable, "-m", "pybevy", "dev", session.scene_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=session.project_dir,
            env=env,
        )
        session.status = "starting"
        logger.debug(
            "Spawned process pid=%d for session %s",
            session.process.pid,
            session.session_id,
        )

    def _kill_process(self, session: Session) -> None:
        proc = session.process
        if proc is None:
            return

        try:
            proc.terminate()
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            with contextlib.suppress(subprocess.TimeoutExpired):
                proc.wait(timeout=2)
        except Exception:
            logger.debug("Error killing process for session %s", session.session_id, exc_info=True)

        session.process = None

    def start_monitor(self) -> None:
        self._monitor_stop.clear()
        if self._monitor_thread is not None:
            return
        self._monitor_running = True
        self._monitor_thread = threading.Thread(target=self._monitor_loop, daemon=True)
        self._monitor_thread.start()
        logger.debug("Monitor thread started")

    def stop_monitor(self) -> None:
        self._monitor_running = False
        self._monitor_stop.set()
        if self._monitor_thread is not None:
            self._monitor_thread.join(timeout=5)
            self._monitor_thread = None

    def _monitor_loop(self) -> None:
        while self._monitor_running:
            try:
                self._monitor_tick()
            except Exception:
                logger.exception("Monitor tick error")
            # Event-based wait so stop_monitor() returns promptly instead of
            # blocking up to 2s for time.sleep to elapse.
            if self._monitor_stop.wait(2.0):
                break

    def _monitor_tick(self) -> None:
        with self._lock:
            sessions = list(self._sessions.values())

        for session in sessions:
            proc = session.process
            if proc is None:
                continue

            exit_code = proc.poll()

            if session.status == "starting" and exit_code is None:
                if self._health_check(session.port):
                    session.status = "running"
                    logger.info("Session %s is now running", session.session_id)

            elif exit_code is not None and session.status in ("starting", "running"):
                session.status = "crashed"
                logger.warning(
                    "Session %s crashed (exit code %d, restarts %d/%d)",
                    session.session_id,
                    exit_code,
                    session.restart_count,
                    session.max_restarts,
                )

                if session.restart_count < session.max_restarts:
                    session.restart_count += 1
                    session.status = "starting"
                    self._spawn_process(session)
                    logger.info(
                        "Auto-restarted session %s (attempt %d)",
                        session.session_id,
                        session.restart_count,
                    )

    @staticmethod
    def _health_check(port: int) -> bool:
        try:
            import httpx  # noqa: PLC0415

            resp = httpx.get(f"http://127.0.0.1:{port}/health", timeout=2.0)
            return resp.status_code == 200
        except Exception:
            return False
