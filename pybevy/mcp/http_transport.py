"""MCP Streamable HTTP transport.

Implements the MCP Streamable HTTP transport (spec 2025-03-26) using stdlib
http.server. Zero new dependencies.

Endpoint: POST/GET/DELETE /mcp
- POST: JSON-RPC request -> application/json or text/event-stream (SSE)
- GET: 405 (no server-push needed)
- DELETE: Terminate session via Mcp-Session-Id header
"""

from __future__ import annotations

import json
import logging
import signal
import threading
import time
import uuid
from dataclasses import dataclass, field
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any

from .bridge import McpBridge

type JsonDict = dict[str, Any]

logger = logging.getLogger("pybevy.mcp.http")

_SESSION_EXPIRY_SECONDS = 30 * 60  # 30 minutes
_SWEEP_INTERVAL_SECONDS = 60


@dataclass
class McpSession:
    """A single MCP session with its own bridge instance."""

    session_id: str
    bridge: McpBridge
    lock: threading.Lock = field(default_factory=threading.Lock)
    last_access: float = field(default_factory=time.monotonic)


class SessionStore:
    """Thread-safe store of active MCP sessions."""

    def __init__(self, *, record: bool = False) -> None:
        self._sessions: dict[str, McpSession] = {}
        self._lock = threading.Lock()
        self._record = record

    def create(self) -> McpSession:
        session_id = uuid.uuid4().hex[:16]
        bridge = McpBridge(protocol_version="2025-03-26", record=self._record)
        session = McpSession(session_id=session_id, bridge=bridge)
        with self._lock:
            self._sessions[session_id] = session
        logger.info("Created session %s", session_id)
        return session

    def get(self, session_id: str) -> McpSession | None:
        with self._lock:
            session = self._sessions.get(session_id)
        if session is not None:
            session.last_access = time.monotonic()
        return session

    def remove(self, session_id: str) -> McpSession | None:
        with self._lock:
            session = self._sessions.pop(session_id, None)
        if session is not None:
            logger.info("Removed session %s", session_id)
            if session.bridge._recorder:
                session.bridge._recorder.close()
            session.bridge._stop_subprocess()
            session.bridge._cleanup_hub_session()
        return session

    def sweep_expired(self) -> None:
        now = time.monotonic()
        expired: list[str] = []
        with self._lock:
            for sid, session in self._sessions.items():
                if now - session.last_access > _SESSION_EXPIRY_SECONDS:
                    expired.append(sid)
        for sid in expired:
            logger.info("Expiring idle session %s", sid)
            self.remove(sid)

    def shutdown_all(self) -> None:
        with self._lock:
            session_ids = list(self._sessions.keys())
        for sid in session_ids:
            self.remove(sid)


def _format_sse_event(data: JsonDict) -> str:
    """Format a JSON-RPC message as an SSE event."""
    payload = json.dumps(data, separators=(",", ":"))
    return f"event: message\ndata: {payload}\n\n"


class McpHttpHandler(BaseHTTPRequestHandler):
    """HTTP handler implementing MCP Streamable HTTP transport."""

    store: SessionStore

    def log_message(self, format: str, *args: object) -> None:
        logger.debug(format, *args)

    def do_POST(self) -> None:
        if self.path != "/mcp":
            self._error(404, "Not found")
            return

        accept = self.headers.get("Accept", "")
        if "application/json" not in accept:
            self._error(406, "Must Accept application/json")
            return

        content_length = int(self.headers.get("Content-Length", 0))
        if content_length == 0:
            self._error(400, "Empty body")
            return
        max_body = 10 * 1024 * 1024  # 10 MB
        if content_length > max_body:
            self._error(413, f"Request body too large (max {max_body} bytes)")
            return

        try:
            raw = self.rfile.read(content_length)
            body = json.loads(raw)
        except (json.JSONDecodeError, ValueError) as e:
            self._json_response(
                200,
                {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": f"Parse error: {e}"}},
            )
            return

        messages: list[JsonDict] = body if isinstance(body, list) else [body]

        session_id = self.headers.get("Mcp-Session-Id")
        session: McpSession | None = None

        is_initialize = (
            len(messages) == 1
            and messages[0].get("method") == "initialize"
            and session_id is None
        )

        if is_initialize:
            session = self.store.create()
        elif session_id is not None:
            session = self.store.get(session_id)
            if session is None:
                self._error(404, "Session expired or unknown")
                return
        else:
            self._error(400, "Missing Mcp-Session-Id header")
            return

        responses: list[JsonDict] = []
        notifications: list[JsonDict] = []

        with session.lock:
            for msg in messages:
                is_notification = "id" not in msg
                result = session.bridge._dispatch(msg)
                if result is not None and not is_notification:
                    responses.append(result)
                notifications.extend(session.bridge.drain_notifications())

        extra_headers: dict[str, str] = {}
        if is_initialize:
            extra_headers["Mcp-Session-Id"] = session.session_id

        if not responses and not notifications:
            self.send_response(202)
            for k, v in extra_headers.items():
                self.send_header(k, v)
            self.end_headers()
            return

        if responses and not notifications:
            data = responses[0] if len(responses) == 1 else responses
            self._json_response(200, data, extra_headers=extra_headers)
            return

        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        for k, v in extra_headers.items():
            self.send_header(k, v)
        self.end_headers()

        for resp in responses:
            self.wfile.write(_format_sse_event(resp).encode("utf-8"))
        for notif in notifications:
            self.wfile.write(_format_sse_event(notif).encode("utf-8"))
        self.wfile.flush()

    def do_GET(self) -> None:
        if self.path == "/mcp":
            self._error(405, "GET not supported; use POST")
        else:
            self._error(404, "Not found")

    def do_DELETE(self) -> None:
        if self.path != "/mcp":
            self._error(404, "Not found")
            return

        session_id = self.headers.get("Mcp-Session-Id")
        if not session_id:
            self._error(400, "Missing Mcp-Session-Id header")
            return

        session = self.store.remove(session_id)
        if session is None:
            self._error(404, "Session not found")
            return

        self._json_response(200, {"status": "session_terminated"})

    def _json_response(
        self,
        status: int,
        data: object,
        extra_headers: dict[str, str] | None = None,
    ) -> None:
        body = json.dumps(data, indent=2).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        if extra_headers:
            for k, v in extra_headers.items():
                self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)

    def _error(self, status: int, message: str) -> None:
        self._json_response(status, {"error": message})


def run_mcp_http(host: str = "127.0.0.1", port: int = 8080, *, record: bool = False) -> None:
    """Start the MCP Streamable HTTP transport server."""
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(name)s] %(levelname)s: %(message)s",
        datefmt="%H:%M:%S",
    )

    store = SessionStore(record=record)
    McpHttpHandler.store = store

    stop_event = threading.Event()

    def _sweep_loop() -> None:
        while not stop_event.is_set():
            stop_event.wait(_SWEEP_INTERVAL_SECONDS)
            if not stop_event.is_set():
                store.sweep_expired()

    sweeper = threading.Thread(target=_sweep_loop, daemon=True)
    sweeper.start()

    server = ThreadingHTTPServer((host, port), McpHttpHandler)
    logger.info("MCP Streamable HTTP listening on http://%s:%d/mcp", host, port)

    def _handle_sigterm(signum: int, frame: object) -> None:
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, _handle_sigterm)

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        logger.info("Shutting down...")
    finally:
        stop_event.set()
        store.shutdown_all()
        server.server_close()
