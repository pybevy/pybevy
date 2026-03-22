"""HTTP server for the PyBevy Hub.

Provides a REST API for managing PyBevy engine sessions.
Uses stdlib http.server — zero additional dependencies.
"""

from __future__ import annotations

import json
import logging
import re
import signal
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any

from pybevy.mcp.hub.session import SessionManager

logger = logging.getLogger("pybevy.hub")

type JsonDict = dict[str, Any]

_SESSION_ID_RE = re.compile(r"^/sessions/([a-zA-Z0-9_-]+)$")
_SESSION_RESTART_RE = re.compile(r"^/sessions/([a-zA-Z0-9_-]+)/restart$")


class HubHandler(BaseHTTPRequestHandler):
    """HTTP request handler for Hub session management."""

    manager: SessionManager

    def log_message(self, format: str, *args: object) -> None:
        logger.debug(format, *args)

    def do_GET(self) -> None:
        path = self.path

        if path == "/health":
            sessions = self.manager.list_sessions()
            self._json_response(200, {"status": "ok", "sessions": len(sessions)})
            return

        if path == "/sessions":
            sessions = self.manager.list_sessions()
            self._json_response(200, {"sessions": [s.to_dict() for s in sessions]})
            return

        m = _SESSION_ID_RE.match(path)
        if m:
            session = self.manager.get_session(m.group(1))
            if session is None:
                self._error(404, "Session not found")
            else:
                self._json_response(200, session.to_dict())
            return

        self._error(404, "Not found")

    def do_POST(self) -> None:
        path = self.path

        if path == "/sessions":
            body = self._read_json()
            if body is None:
                return
            project_dir = body.get("project_dir", ".")
            scene_path = body.get("scene_path", "")
            if not scene_path:
                self._error(400, "Missing 'scene_path'")
                return
            session_id = body.get("session_id")
            try:
                session = self.manager.create_session(
                    project_dir=str(project_dir),
                    scene_path=str(scene_path),
                    session_id=str(session_id) if session_id else None,
                )
                self._json_response(201, session.to_dict())
            except RuntimeError as e:
                self._error(503, str(e))
            return

        m = _SESSION_RESTART_RE.match(path)
        if m:
            restarted = self.manager.restart_session(m.group(1))
            if restarted is None:
                self._error(404, "Session not found")
            else:
                self._json_response(200, restarted.to_dict())
            return

        self._error(404, "Not found")

    def do_DELETE(self) -> None:
        m = _SESSION_ID_RE.match(self.path)
        if m:
            if self.manager.destroy_session(m.group(1)):
                self._json_response(200, {"status": "destroyed"})
            else:
                self._error(404, "Session not found")
            return

        self._error(404, "Not found")

    def _read_json(self) -> JsonDict | None:
        content_length = int(self.headers.get("Content-Length", 0))
        if content_length == 0:
            self._error(400, "Empty request body")
            return None
        try:
            raw = self.rfile.read(content_length)
            return json.loads(raw)
        except (json.JSONDecodeError, ValueError) as e:
            self._error(400, f"Invalid JSON: {e}")
            return None

    def _json_response(self, status: int, data: JsonDict) -> None:
        body = json.dumps(data, indent=2).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _error(self, status: int, message: str) -> None:
        self._json_response(status, {"error": message})


def run_hub(port: int = 8419) -> None:
    """Start the PyBevy Hub HTTP server."""
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(name)s] %(levelname)s: %(message)s",
        datefmt="%H:%M:%S",
    )

    manager = SessionManager()
    manager.start_monitor()
    HubHandler.manager = manager

    server = ThreadingHTTPServer(("127.0.0.1", port), HubHandler)
    logger.info("PyBevy Hub listening on http://127.0.0.1:%d", port)

    def _handle_sigterm(signum: int, frame: object) -> None:
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, _handle_sigterm)

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        logger.info("Shutting down...")
    finally:
        manager.shutdown_all()
        server.server_close()
