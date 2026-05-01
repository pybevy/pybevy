"""MCP stdio bridge for AI integration.

Two-mode dispatch:
  Mode A (no engine): local tools only (get_started, run_scene, search_api, get_type_definition)
  Mode B (engine running): forward via HTTP REST to ControlPlugin, inject bridge-only tools
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import threading
import time
from typing import Any

import httpx

from . import ApiIndex
from .definitions import (
    builtin_prompts,
    builtin_tools,
    filtered_resources,
)
from .engine import build_engine_env, find_free_port, has_display
from .recorder import SessionRecorder

type JsonId = int | str | None
type JsonDict = dict[str, Any]


def _log(msg: str) -> None:
    sys.stderr.write(msg + "\n")
    sys.stderr.flush()


def _is_log_error_line(line: str) -> bool:
    """Check if line is a tracing/log ERROR line (e.g. 'ERROR bevy_render::...')."""
    # Strip optional timestamp prefix like "2024-01-01T12:00:00.000Z "
    stripped = line.lstrip()
    # Match "ERROR " at start, or after timestamp-like prefix
    if stripped.startswith("ERROR "):
        return True
    # Handle timestamped lines: "2024-... ERROR ..."
    parts = stripped.split(None, 2)
    return len(parts) >= 2 and parts[1] == "ERROR"


LOAD_SCENE_TOOL: JsonDict = {
    "name": "run_scene",
    "description": (
        "Start a PyBevy scene. Launches (or restarts) the Bevy app subprocess "
        "with hot-reload. Only call this ONCE to start the scene. "
        "After that, just edit the .py file and use reload or reload_and_capture. "
        "Do NOT call run_scene again unless switching to a different scene file."
    ),
    "inputSchema": {
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Path to the Python scene file (e.g. 'examples/mcp/mcp_scene.py')",
            },
            "headless": {
                "type": "boolean",
                "description": "Run without a display (no window). The scene must disable WinitPlugin. Bypasses the display check.",
                "default": False,
            },
        },
        "required": ["path"],
    },
}

GET_LOGS_TOOL: JsonDict = {
    "name": "get_logs",
    "description": (
        "Get recent Bevy subprocess logs (stderr output). Use errors_only=true "
        "to see only Python tracebacks and errors. Call this after reload "
        "to check for errors, or anytime to see print() output from systems."
    ),
    "inputSchema": {
        "type": "object",
        "properties": {
            "lines": {
                "type": "integer",
                "description": "Number of recent log lines to return (default 50, max 100)",
                "default": 50,
            },
            "errors_only": {
                "type": "boolean",
                "description": "Only return Python errors/tracebacks (default false)",
                "default": False,
            },
        },
    },
}


_LONG_TIMEOUT_TOOLS = {"capture_timeline", "capture_turnaround", "reload_and_capture", "capture_depth", "schedule_actions"}

_SCREENSHOT_TOOLS = {
    "capture_screenshot", "capture_timeline",
    "capture_turnaround", "capture_depth", "reload_and_capture",
}

_SCREENSHOT_DIR: str | None = None


def _get_screenshot_dir() -> str:
    """Get or create the screenshot output directory."""
    global _SCREENSHOT_DIR
    if _SCREENSHOT_DIR is None:
        import tempfile  # noqa: PLC0415

        _SCREENSHOT_DIR = tempfile.mkdtemp(prefix="pybevy-screenshots-")
        _log(f"[MCP Bridge] Screenshot dir: {_SCREENSHOT_DIR}")
    return _SCREENSHOT_DIR


def _save_screenshot_to_file(base64_data: str, tool_name: str) -> str | None:
    """Decode base64 PNG and save to a temp file. Returns the file path."""
    import base64  # noqa: PLC0415

    try:
        screenshot_dir = _get_screenshot_dir()
        timestamp = int(time.time() * 1000)
        filename = f"{tool_name}_{timestamp}.png"
        file_path = os.path.join(screenshot_dir, filename)

        png_bytes = base64.b64decode(base64_data)
        with open(file_path, "wb") as f:
            f.write(png_bytes)

        _log(f"[MCP Bridge] Saved screenshot: {file_path} ({len(png_bytes)} bytes)")
        return file_path
    except Exception as e:
        _log(f"[MCP Bridge] Failed to save screenshot: {e}")
        return None


# REST routing for tools that use simple method + path (no path interpolation needed).
# Tools requiring entity/component path segments are handled by explicit branches
# in _call_rest_api() instead.
_TOOL_TO_REST: dict[str, tuple[str, str]] = {
    "query_entities": ("POST", "/api/v1/query"),
    "capture_timeline": ("POST", "/api/v1/screenshot/timeline"),
    "capture_turnaround": ("POST", "/api/v1/screenshot/turnaround"),
    "capture_depth": ("POST", "/api/v1/screenshot/depth"),
    "reload": ("POST", "/api/v1/reload"),
    "get_reload_status": ("GET", "/api/v1/reload/status"),
    "get_last_error": ("GET", "/api/v1/error"),
    "spawn_entity": ("POST", "/api/v1/entities"),
    "batch": ("POST", "/api/v1/batch"),
    "get_scene_summary": ("GET", "/api/v1/scene/summary"),
    "reload_and_capture": ("POST", "/api/v1/reload/capture"),
    "pause_time": ("POST", "/api/v1/time/pause"),
    "resume_time": ("POST", "/api/v1/time/resume"),
    "set_time_scale": ("POST", "/api/v1/time/scale"),
    "get_time_status": ("GET", "/api/v1/time"),
    "seek_time": ("POST", "/api/v1/time/seek"),
    "run_code": ("POST", "/api/v1/execute"),
    "get_performance": ("GET", "/api/v1/performance"),
    "get_registry": ("GET", "/api/v1/debug/registry"),
    "schedule_actions": ("POST", "/api/v1/schedule"),
}

class McpBridge:
    """Stdio MCP bridge that manages a Bevy engine and dispatches JSON-RPC."""

    def __init__(
        self,
        scene_path: str | None = None,
        protocol_version: str = "2024-11-05",
        *,
        record: bool = False,
    ) -> None:
        self._scene_path = scene_path
        self._protocol_version = protocol_version
        self._recorder: SessionRecorder | None = SessionRecorder() if record else None
        self._subprocess: subprocess.Popen[bytes] | None = None
        self._subprocess_port: int | None = None
        self._stderr_lines: list[str] = []
        self._stderr_lock = threading.Lock()
        self._stderr_thread: threading.Thread | None = None
        self._hub_session_id: str | None = None
        self._hub_port: int | None = None
        self._pending_notifications: list[JsonDict] = []

        self._tools = builtin_tools()

        # Load Rust ApiIndex (finds pybevy stubs + guides automatically)
        try:
            self._api_index: ApiIndex | None = ApiIndex()
        except Exception:
            self._api_index = None

        self._instructions = ""
        if self._api_index:
            instructions = self._api_index.get_instructions()
            if instructions:
                self._instructions = instructions

        self._prompts = builtin_prompts(self._instructions)

    def drain_notifications(self) -> list[JsonDict]:
        """Return and clear any buffered server-initiated notifications."""
        notifications = self._pending_notifications
        self._pending_notifications = []
        return notifications

    @property
    def _engine_port(self) -> int:
        if self._hub_port is not None:
            return self._hub_port
        if self._subprocess_port is not None:
            return self._subprocess_port
        return 8420

    @property
    def _base_url(self) -> str:
        return f"http://127.0.0.1:{self._engine_port}"

    def run(self) -> None:
        """Main loop: read stdin, dispatch, write stdout."""
        if self._scene_path:
            self._start_subprocess(self._scene_path)

        try:
            for line in sys.stdin:
                line = line.strip()
                if not line:
                    continue

                try:
                    request: JsonDict = json.loads(line)
                except json.JSONDecodeError as e:
                    self._write_response(
                        {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": f"Parse error: {e}"}}
                    )
                    continue

                response = self._dispatch(request)
                if response is not None:
                    self._write_response(response)
                for notification in self.drain_notifications():
                    self._write_response(notification)
        except KeyboardInterrupt:
            pass
        finally:
            if self._recorder:
                self._recorder.close()
            self._stop_subprocess()
            self._cleanup_hub_session()

    def _has_engine(self) -> bool:
        if self._hub_session_id is not None:
            return True
        return self._subprocess is not None and self._subprocess.poll() is None

    def _dispatch(self, request: JsonDict) -> JsonDict | None:
        t0 = time.monotonic()
        response = self._dispatch_inner(request)
        duration_ms = (time.monotonic() - t0) * 1000

        if self._recorder:
            self._recorder.record(request, response, duration_ms)

        return response

    def _dispatch_inner(self, request: JsonDict) -> JsonDict | None:
        method = str(request.get("method", ""))
        req_id: JsonId = request.get("id")  # type: ignore[assignment]
        params: JsonDict = request.get("params") or {}  # type: ignore[assignment]

        if method == "initialize":
            return self._handle_initialize(req_id)
        if method in ("initialized", "ping"):
            return self._success(req_id, {})
        if method == "notifications/initialized":
            return None

        if self._has_engine():
            return self._dispatch_mode_b(method, req_id, params)
        return self._dispatch_mode_a(method, req_id, params)

    def _dispatch_mode_a(self, method: str, req_id: JsonId, params: JsonDict) -> JsonDict | None:
        """Mode A: no engine. Only bridge-local tools."""
        if method == "tools/list":
            return self._handle_tools_list_local(req_id)
        if method == "tools/call":
            return self._handle_tools_call_local(req_id, params)
        if method == "resources/list":
            return self._handle_resources_list_local(req_id)
        if method == "resources/read":
            return self._handle_resources_read_local(req_id, params)
        if method == "prompts/list":
            return self._handle_prompts_list(req_id)
        if method == "prompts/get":
            return self._handle_prompts_get(req_id, params)
        return self._error(req_id, -32601, f"Method not found: {method}")

    def _dispatch_mode_b(self, method: str, req_id: JsonId, params: JsonDict) -> JsonDict | None:
        """Mode B: engine running. Forward tool calls via HTTP."""
        if method == "tools/list":
            return self._handle_tools_list_full(req_id)
        if method == "tools/call":
            return self._handle_tools_call_forwarded(req_id, params)
        if method == "resources/list":
            return self._handle_resources_list_full(req_id)
        if method == "resources/read":
            return self._handle_resources_read_mode_b(req_id, params)
        if method == "prompts/list":
            return self._handle_prompts_list_full(req_id)
        if method == "prompts/get":
            return self._handle_prompts_get_full(req_id, params)
        return self._error(req_id, -32601, f"Method not found: {method}")

    def _handle_initialize(self, req_id: JsonId) -> JsonDict:
        return self._success(
            req_id,
            {
                "protocolVersion": self._protocol_version,
                "capabilities": {
                    "tools": {"listChanged": True},
                    "resources": {"subscribe": False, "listChanged": True},
                    "prompts": {},
                    "logging": {},
                },
                "serverInfo": {"name": "pybevy-mcp", "version": "0.1.0"},
                "instructions": self._instructions,
            },
        )

    def _handle_tools_list_local(self, req_id: JsonId) -> JsonDict:
        # Expose ALL tools even without engine — some MCP clients (Codex, Gemini)
        # don't support notifications/tools/list_changed, so they'd never see
        # engine tools if we only added them after run_scene.
        return self._handle_tools_list_full(req_id)

    def _handle_tools_call_local(self, req_id: JsonId, params: JsonDict) -> JsonDict | None:
        tool_name = str(params.get("name", ""))
        arguments: JsonDict = params.get("arguments") or {}  # type: ignore[assignment]

        if tool_name == "get_started":
            return self._handle_get_started(req_id, arguments)
        if tool_name == "run_scene":
            return self._handle_run_scene(req_id, arguments)
        if tool_name == "get_logs":
            return self._handle_get_logs(req_id, arguments)
        if tool_name == "search_api":
            return self._handle_search_api(req_id, arguments)
        if tool_name == "get_type_definition":
            return self._handle_get_type_definition(req_id, arguments)
        return self._error(
            req_id, -32603, "No scene loaded. Use the 'run_scene' tool first to start a Bevy app."
        )

    def _handle_resources_list_local(self, req_id: JsonId) -> JsonDict:
        # Expose all resources including scene:// — clients that don't support
        # list_changed would never see them otherwise.
        return self._handle_resources_list_full(req_id)

    def _handle_resources_read_local(self, req_id: JsonId, params: JsonDict) -> JsonDict:
        uri = str(params.get("uri", ""))

        if uri == "guide://index" and self._api_index:
            text = self._api_index.get_guide_index()
            return self._success(
                req_id,
                {"contents": [{"uri": uri, "mimeType": "application/json", "text": text}]},
            )

        if uri.startswith("guide://"):
            name = uri[len("guide://"):]
            if self._api_index:
                content = self._api_index.get_guide(name)
                if content:
                    text = json.dumps({"name": name, "content": content}, indent=2)
                    return self._success(
                        req_id,
                        {"contents": [{"uri": uri, "mimeType": "application/json", "text": text}]},
                    )

        if uri == "api://index" and self._api_index:
            text = self._api_index.get_index()
            return self._success(
                req_id,
                {"contents": [{"uri": uri, "mimeType": "application/json", "text": text}]},
            )

        if uri.startswith("api://module/") and self._api_index:
            module_name = uri[len("api://module/"):]
            module_content = self._api_index.get_module_content(module_name)
            if module_content:
                text = json.dumps({"module": module_name, "content": module_content}, indent=2)
                return self._success(
                    req_id,
                    {"contents": [{"uri": uri, "mimeType": "application/json", "text": text}]},
                )

        if uri.startswith("scene://"):
            return self._error(
                req_id, -32603, "No scene loaded. Use the 'run_scene' tool first to start a Bevy app."
            )

        return self._error(req_id, -32602, f"Unknown resource: {uri}")

    def _handle_prompts_list(self, req_id: JsonId) -> JsonDict:
        prompts = [{"name": p["name"], "description": p["description"], "arguments": []} for p in self._prompts]
        return self._success(req_id, {"prompts": prompts})

    def _handle_prompts_get(self, req_id: JsonId, params: JsonDict) -> JsonDict:
        name = str(params.get("name", ""))
        for p in self._prompts:
            if p["name"] == name:
                content = str(p.get("content", ""))
                return self._success(
                    req_id,
                    {"messages": [{"role": "user", "content": {"type": "text", "text": content}}]},
                )
        return self._error(req_id, -32602, f"Unknown prompt: {name}")

    def _handle_prompts_list_full(self, req_id: JsonId) -> JsonDict:
        prompts = [{"name": p["name"], "description": p["description"], "arguments": []} for p in self._prompts]
        return self._success(req_id, {"prompts": prompts})

    def _handle_prompts_get_full(self, req_id: JsonId, params: JsonDict) -> JsonDict:
        return self._handle_prompts_get(req_id, params)

    def _handle_tools_list_full(self, req_id: JsonId) -> JsonDict:
        tools: list[JsonDict] = []
        for t in self._tools:
            tools.append({k: v for k, v in t.items() if k != "feature_gate"})
        tools.append(LOAD_SCENE_TOOL)
        tools.append(GET_LOGS_TOOL)
        return self._success(req_id, {"tools": tools})

    def _handle_resources_list_full(self, req_id: JsonId) -> JsonDict:
        resources = []
        for res in filtered_resources(api_discovery=True):
            resources.append({
                "uri": res.get("uri", ""),
                "name": res.get("name", ""),
                "description": res.get("description", ""),
                "mimeType": res.get("mimeType", "text/plain"),
            })
        return self._success(req_id, {"resources": resources})

    def _handle_resources_read_mode_b(self, req_id: JsonId, params: JsonDict) -> JsonDict:
        uri = str(params.get("uri", ""))
        if uri.startswith("scene://"):
            return self._forward_scene_resource(req_id, uri)
        return self._handle_resources_read_local(req_id, params)

    def _handle_tools_call_forwarded(self, req_id: JsonId, params: JsonDict) -> JsonDict | None:
        tool_name = str(params.get("name", ""))
        arguments: JsonDict = params.get("arguments") or {}  # type: ignore[assignment]

        if tool_name == "get_started":
            return self._handle_get_started(req_id, arguments)
        if tool_name == "run_scene":
            return self._handle_run_scene(req_id, arguments)
        if tool_name == "get_logs":
            return self._handle_get_logs(req_id, arguments)
        if tool_name == "search_api":
            return self._handle_search_api(req_id, arguments)
        if tool_name == "get_type_definition":
            return self._handle_get_type_definition(req_id, arguments)

        if tool_name in ("reload", "reload_and_capture"):
            with self._stderr_lock:
                self._stderr_lines.clear()

        if tool_name == "schedule_actions":
            # Validate that no action uses a bridge-local tool
            _bridge_local = {"schedule_actions", "get_schedule_result", "run_scene", "get_started", "get_logs", "search_api", "get_type_definition"}
            for action in arguments.get("actions", []):
                tool = action.get("tool", "")
                if tool in _bridge_local:
                    return self._error(req_id, -32602, f"Tool '{tool}' cannot be used inside a schedule (bridge-local tool)")
            # Dynamic timeout based on max 'at' value in actions
            max_at = max((a.get("at", 0) for a in arguments.get("actions", [])), default=0)
            timeout = max(max_at + 60.0, 120.0)
        elif tool_name in _LONG_TIMEOUT_TOOLS:
            timeout = 120.0
        else:
            timeout = 30.0
        return self._forward_tool_via_http(req_id, tool_name, arguments, timeout)

    def _forward_tool_via_http(
        self, req_id: JsonId, tool_name: str, arguments: JsonDict, timeout: float = 30.0
    ) -> JsonDict:
        """Forward a tool call to the engine's REST API."""

        coerced = self._coerce_arguments(tool_name, arguments)

        try:
            result = self._call_rest_api(tool_name, coerced, timeout)

            # Check for pause warning injected by engine
            time_paused = False
            if isinstance(result, dict):
                time_paused = result.pop("_time_paused", False)

            content = self._format_tool_result(tool_name, result)

            if time_paused:
                content.insert(
                    0,
                    {
                        "type": "text",
                        "text": "NOTE: Scene time is currently PAUSED. Animation and time-dependent systems are frozen. Use resume_time to unpause.",
                    },
                )

            # After reload tools, check stderr for Bevy engine errors
            # (asset loading failures, render errors, etc.) that aren't
            # captured by LastSystemError.
            if tool_name in ("reload_and_capture", "reload"):
                time.sleep(0.1)  # Let stderr reader thread catch up
                engine_errors = self._check_stderr_for_errors()
                if engine_errors:
                    content.insert(
                        0,
                        {
                            "type": "text",
                            "text": f"⚠ ENGINE ERRORS DETECTED:\n{engine_errors}",
                        },
                    )

            return self._success(req_id, {"content": content})
        except httpx.TimeoutException:
            hint = ""
            _time_tools = {"pause_time", "resume_time", "set_time_scale", "get_time_status", "seek_time"}
            if tool_name not in _time_tools:
                hint = " Hint: if scene time is paused or the window is minimized, the engine may stop processing requests. Try resume_time or focus the window."
            return self._error(req_id, -32603, f"Engine timeout ({timeout:.0f}s) for {tool_name}.{hint}")
        except httpx.ConnectError:
            return self._error(req_id, -32603, "Engine not responding. Use 'run_scene' to restart.")
        except httpx.HTTPStatusError as e:
            # Extract structured error message from JSON response body
            try:
                error_json = e.response.json()
                msg = error_json.get("error", str(e))
            except Exception:
                msg = str(e)
            return self._error(req_id, -32603, msg)
        except Exception as e:
            return self._error(req_id, -32603, f"HTTP error: {e}")

    def _coerce_arguments(self, tool_name: str, arguments: JsonDict) -> JsonDict:
        """Coerce string arguments to their schema types (MCP clients may send strings)."""
        tool_def = next((t for t in self._tools if t["name"] == tool_name), None)
        if tool_def is None:
            return arguments

        props = tool_def.get("inputSchema", {}).get("properties", {})
        coerced = dict(arguments)
        for key, value in coerced.items():
            if key not in props or not isinstance(value, str):
                continue
            raw_type = props[key].get("type", "")
            types = set(raw_type) if isinstance(raw_type, list) else {raw_type}
            try:
                if "integer" in types:
                    coerced[key] = int(value)
                elif "number" in types:
                    coerced[key] = float(value)
                elif "boolean" in types:
                    coerced[key] = value.lower() in ("true", "1", "yes")
            except (ValueError, TypeError):
                pass
        return coerced

    @staticmethod
    def _format_tool_result(tool_name: str, result: JsonDict) -> list[JsonDict]:
        """Format engine response as MCP content blocks, extracting images.

        Screenshots are saved to temp files and returned as MCP image content
        blocks. This works with Claude Code natively and avoids token-limit
        issues with large base64 payloads.
        """
        # Schedule results: extract images from action results
        if tool_name in ("schedule_actions", "get_schedule_result"):
            return McpBridge._format_schedule_result(result)

        if tool_name not in _SCREENSHOT_TOOLS:
            return [{"type": "text", "text": json.dumps(result, indent=2)}]

        content: list[JsonDict] = []

        # reload_and_capture: image is in "screenshot" key
        # Other screenshot tools: image is in "image" key
        image_key = "screenshot" if tool_name in ("reload_and_capture", "capture_depth") else "image"
        image_data = result.get(image_key)

        if isinstance(image_data, str) and len(image_data) > 100:
            # Save to temp file for universal client compatibility
            file_path = _save_screenshot_to_file(image_data, tool_name)

            if file_path:
                # Return as MCP image content block (spec-compliant)
                content.append({"type": "image", "data": image_data, "mimeType": "image/png"})

                # Include metadata + file path
                metadata = {k: v for k, v in result.items() if k != image_key}
                metadata["saved_to"] = file_path
                content.append({"type": "text", "text": json.dumps(metadata, indent=2)})
            else:
                # Fallback: image content block without file
                content.append({"type": "image", "data": image_data, "mimeType": "image/png"})
                metadata = {k: v for k, v in result.items() if k != image_key}
                if metadata:
                    content.append({"type": "text", "text": json.dumps(metadata, indent=2)})
        else:
            # No image or small data — return as text
            content.append({"type": "text", "text": json.dumps(result, indent=2)})

        return content

    @staticmethod
    def _format_schedule_result(result: JsonDict) -> list[JsonDict]:
        """Extract images from schedule action results into MCP content blocks."""
        content: list[JsonDict] = []
        actions = result.get("results", [])
        image_indices: list[int] = []

        for action_result in actions:
            if not isinstance(action_result, dict):
                continue
            inner = action_result.get("result")
            if not isinstance(inner, dict):
                continue
            # Check for image in standard screenshot keys
            for key in ("image", "screenshot"):
                image_data = inner.get(key)
                if isinstance(image_data, str) and len(image_data) > 100:
                    label = action_result.get("label") or f"action_{action_result.get('index', '?')}"
                    file_path = _save_screenshot_to_file(image_data, f"schedule_{label}")
                    content.append({"type": "image", "data": image_data, "mimeType": "image/png"})
                    if file_path:
                        content.append({"type": "text", "text": f"[{label}] saved to {file_path}"})
                    image_indices.append(action_result.get("index", -1))
                    # Remove large base64 from the JSON to avoid token bloat
                    inner[key] = "<extracted to MCP image block>"
                    break

        # Add the full result JSON (with images replaced by placeholders)
        content.append({"type": "text", "text": json.dumps(result, indent=2)})
        return content

    def _call_rest_api(self, tool_name: str, arguments: JsonDict, timeout: float = 30.0) -> JsonDict:
        """Map tool name + arguments to REST API call."""

        base = self._base_url
        entity_ref = str(arguments.get("entity", ""))

        if tool_name == "get_component":
            component = arguments.get("component", "")
            url = f"{base}/api/v1/entities/{entity_ref}/components/{component}"
            resp = httpx.get(url, timeout=timeout)
        elif tool_name == "get_component_schema":
            name = arguments.get("name", "")
            url = f"{base}/api/v1/components/{name}/schema"
            resp = httpx.get(url, timeout=timeout)
        elif tool_name == "despawn_entity":
            url = f"{base}/api/v1/entities/{entity_ref}"
            resp = httpx.delete(url, timeout=timeout)
        elif tool_name == "set_component":
            component = arguments.get("component", "")
            url = f"{base}/api/v1/entities/{entity_ref}/components/{component}"
            resp = httpx.put(url, json={"fields": arguments.get("fields", {})}, timeout=timeout)
        elif tool_name == "remove_component":
            component = arguments.get("component", "")
            url = f"{base}/api/v1/entities/{entity_ref}/components/{component}"
            resp = httpx.delete(url, timeout=timeout)
        elif tool_name == "get_bounding_box":
            url = f"{base}/api/v1/entities/{entity_ref}/bounding_box"
            resp = httpx.get(url, timeout=timeout)
        elif tool_name == "set_resource":
            rt = arguments.get("resource_type", "")
            url = f"{base}/api/v1/resources/{rt}"
            resp = httpx.put(url, json={"value": arguments.get("value", {})}, timeout=timeout)
        elif tool_name == "remove_resource":
            rt = arguments.get("resource_type", "")
            url = f"{base}/api/v1/resources/{rt}"
            resp = httpx.delete(url, timeout=timeout)
        elif tool_name == "set_asset":
            entity = arguments.get("entity")
            body = {
                "entity": entity,
                "component": arguments.get("component", ""),
                "asset_type": arguments.get("asset_type", ""),
                "fields": arguments.get("fields", {}),
            }
            url = f"{base}/api/v1/assets/mutate"
            resp = httpx.post(url, json=body, timeout=timeout)
        elif tool_name == "get_schedule_result":
            schedule_id = arguments.get("schedule_id", "")
            url = f"{base}/api/v1/schedule/{schedule_id}"
            resp = httpx.get(url, timeout=timeout)
        elif tool_name == "query_spatial":
            if "radius" in arguments:
                # Neighborhood mode
                entity = arguments.get("entity")
                body = {"entity": entity, "radius": arguments["radius"]}
                if "max_results" in arguments:
                    body["max_results"] = arguments["max_results"]
                url = f"{base}/api/v1/spatial/neighborhood"
            else:
                # Pairwise mode
                entity_a = arguments.get("entity_a")
                entity_b = arguments.get("entity_b")
                body = {"entity_a": entity_a, "entity_b": entity_b}
                url = f"{base}/api/v1/spatial/query"
            resp = httpx.post(url, json=body, timeout=timeout)
        elif tool_name == "check_overlaps":
            entity = arguments.get("entity")
            if entity is not None:
                # Single-entity mode
                body = {"entity": entity}
                if "include_siblings" in arguments:
                    body["include_siblings"] = arguments["include_siblings"]
                if "max_float_gap" in arguments:
                    body["max_float_gap"] = arguments["max_float_gap"]
                if "ground_y" in arguments:
                    body["ground_y"] = arguments["ground_y"]
                url = f"{base}/api/v1/spatial/overlaps"
            else:
                # Scene-wide mode
                body = {}
                for key in ("min_penetration", "max_results", "max_float_gap", "ground_y", "include_siblings"):
                    if key in arguments:
                        body[key] = arguments[key]
                url = f"{base}/api/v1/spatial/overlaps/all"
            resp = httpx.post(url, json=body, timeout=timeout)
        elif tool_name == "capture_screenshot":
            gizmos = arguments.pop("gizmos", False)
            path = "/api/v1/screenshot/gizmos" if gizmos else "/api/v1/screenshot"
            url = f"{base}{path}"
            resp = httpx.post(url, json=arguments, timeout=timeout)
        elif tool_name in _TOOL_TO_REST:
            method, path_template = _TOOL_TO_REST[tool_name]
            url = f"{base}{path_template}"
            if method == "GET":
                resp = httpx.get(url, timeout=timeout)
            elif method == "POST":
                resp = httpx.post(url, json=arguments, timeout=timeout)
            else:
                resp = httpx.request(method, url, json=arguments, timeout=timeout)
        else:
            msg = f"Unknown tool: {tool_name}"
            raise ValueError(msg)

        resp.raise_for_status()
        return resp.json()

    def _forward_scene_resource(self, req_id: JsonId, uri: str) -> JsonDict:
        """Forward scene:// resource reads via HTTP."""

        resource_map: dict[str, str] = {
            "scene://entities": "/api/v1/entities",
            "scene://resources": "/api/v1/resources",
            "scene://systems": "/api/v1/systems",
            "scene://debug": "/api/v1/performance",
        }

        path = resource_map.get(uri)
        if not path:
            return self._error(req_id, -32602, f"Unknown scene resource: {uri}")

        try:
            resp = httpx.get(f"{self._base_url}{path}", timeout=10.0)
            resp.raise_for_status()
            text = json.dumps(resp.json(), indent=2)
            return self._success(
                req_id,
                {"contents": [{"uri": uri, "mimeType": "application/json", "text": text}]},
            )
        except Exception as e:
            return self._error(req_id, -32603, f"Failed to read {uri}: {e}")

    def _handle_get_started(self, req_id: JsonId, arguments: JsonDict | None = None) -> JsonDict:
        key = str((arguments or {}).get("confirmation_key", ""))
        if key == "pybevy-ready":
            text = "Instructions confirmed. Tools unlocked. Start with guide://patterns, then run_scene."
            if not has_display():
                text += (
                    "\n\nNote: No display detected. For headless rendering, use "
                    "run_scene(path=..., headless=True) with a scene that disables WinitPlugin "
                    "and uses RenderTarget.image(). Screenshots will use GPU readback. "
                    "See examples/misc/headless_render.py for reference."
                )
            return self._success(req_id, {"content": [{"type": "text", "text": text}]})

        instructions = self._instructions
        if not instructions:
            instructions = "No instructions available. Read guide://patterns for scene conventions."
        if not has_display():
            instructions += (
                "\n\n---\n**No display detected.** Headless rendering is supported: use "
                "run_scene(path=..., headless=True) with a scene that disables WinitPlugin "
                "and renders to RenderTarget.image(). Screenshots, timelines, and turnarounds "
                "will use GPU readback. See examples/misc/headless_render.py."
            )
        return self._success(req_id, {"content": [{"type": "text", "text": instructions}]})

    def _handle_run_scene(self, req_id: JsonId, arguments: JsonDict) -> JsonDict | None:
        path = str(arguments.get("path", ""))
        headless = bool(arguments.get("headless", False))
        if not path:
            return self._error(req_id, -32602, "Missing 'path' parameter")
        if not os.path.exists(path):
            return self._error(req_id, -32602, f"File not found: {path}")

        if not headless and not has_display():
            # Try the Hub for headless environments
            hub_result = self._try_hub_create_session(path)
            if hub_result is not None:
                if "session_id" not in hub_result or "port" not in hub_result:
                    return self._error(req_id, -32603, "Hub returned incomplete response (missing session_id or port)")
                self._hub_session_id = str(hub_result["session_id"])
                self._hub_port = int(hub_result["port"])
                self._scene_path = path

                for _ in range(10):
                    time.sleep(1.0)
                    if self._health_check():
                        break

                status_parts = [
                    f"Scene loaded via Hub: {path}",
                    f"Engine port: {hub_result['port']} (session: {self._hub_session_id})",
                    "Hot-reload is active: just edit the .py file to update the scene.",
                ]

                if not self._health_check():
                    status_parts.append("WARNING: Engine not yet responding. It may still be starting up.")

                self._pending_notifications.append({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"})
                self._pending_notifications.append({"jsonrpc": "2.0", "method": "notifications/resources/list_changed"})
                return self._success(
                    req_id, {"content": [{"type": "text", "text": "\n".join(status_parts)}]}
                )

            return self._success(
                req_id,
                {"content": [{"type": "text", "text": (
                    "No display available (DISPLAY / WAYLAND_DISPLAY not set or invalid).\n\n"
                    "Scene tools (run_scene, screenshot, spawn, etc.) require a display.\n"
                    "You can still use guide://, api://, search_api, and get_type_definition.\n\n"
                    "To enable scene tools:\n"
                    "  1. Run `pybevy dev <scene.py>` in a terminal with display access.\n"
                    "  2. Or run `pybevy hub` and retry run_scene.\n"
                )}]},
            )

        self._stop_subprocess()
        self._scene_path = path
        self._start_subprocess(path)

        time.sleep(2.0)

        proc = self._subprocess
        if proc is not None and proc.poll() is not None:
            exit_code = proc.returncode
            stderr_output = self._check_stderr_for_errors() or self._get_recent_stderr()
            error_msg = f"Bevy subprocess crashed on startup (exit code {exit_code})"
            if stderr_output:
                error_msg += f"\n\nSubprocess stderr:\n{stderr_output}"
            return self._error(req_id, -32603, error_msg)

        status_parts = [
            f"Scene loaded: {path}",
            "PyBevy app starting with hot-reload enabled.",
            "Hot-reload is active: just edit the .py file to update the scene. Do NOT call run_scene again.",
            "Workflow: edit the .py file -> reload or reload_and_capture -> verify screenshot -> iterate.",
        ]

        stderr_errors = self._check_stderr_for_errors()
        if stderr_errors:
            status_parts.append(f"\nWARNING — Python errors detected during startup:\n{stderr_errors}")

        self._pending_notifications.append({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"})
        self._pending_notifications.append({"jsonrpc": "2.0", "method": "notifications/resources/list_changed"})
        return self._success(
            req_id, {"content": [{"type": "text", "text": "\n".join(status_parts)}]}
        )

    def _handle_search_api(self, req_id: JsonId, arguments: JsonDict) -> JsonDict:
        query = str(arguments.get("query", ""))
        if not query:
            return self._error(req_id, -32602, "Missing 'query' parameter")
        if not self._api_index:
            return self._error(req_id, -32603, "ApiIndex not available")

        results = self._api_index.search(query)
        text = results
        if results and results.strip() not in ("[]", ""):
            text += "\n\nTip: Check guide://index for curated topic guides (faster than API search). Most types available via `from pybevy.prelude import *`. Use get_type_definition('ClassName') for full definitions."
        return self._success(req_id, {"content": [{"type": "text", "text": text}]})

    def _handle_get_type_definition(self, req_id: JsonId, arguments: JsonDict) -> JsonDict:
        type_name = str(arguments.get("type_name", ""))
        if not type_name:
            return self._error(req_id, -32602, "Missing 'type_name' parameter")
        if not self._api_index:
            return self._error(req_id, -32603, "ApiIndex not available")

        structured = self._api_index.get_type_definition_structured(type_name)
        if structured:
            result = json.dumps({"type_name": type_name, "definition": json.loads(structured)}, indent=2)
        else:
            result = json.dumps({"type_name": type_name, "error": "Type not found in stubs"}, indent=2)

        tip = "Tip: For scene patterns and code templates, read guide://patterns. For topic-specific docs (lighting, materials, camera), check guide://index."
        return self._success(
            req_id,
            {"content": [{"type": "text", "text": result}, {"type": "text", "text": tip}]},
        )

    def _handle_get_logs(self, req_id: JsonId, arguments: JsonDict) -> JsonDict:
        lines = int(arguments.get("lines", 50))  # type: ignore[call-overload]
        errors_only = bool(arguments.get("errors_only", False))

        if self._subprocess is None:
            return self._error(req_id, -32603, "No scene loaded. Use 'run_scene' first.")

        if errors_only:
            output = self._check_stderr_for_errors()
            if not output:
                output = "No errors detected."
        else:
            output = self._get_recent_stderr(max_lines=min(lines, 100))
            if not output:
                output = "(no output captured yet)"

        return self._success(req_id, {"content": [{"type": "text", "text": output}]})

    def _try_hub_create_session(self, scene_path: str) -> JsonDict | None:

        try:
            resp = httpx.post(
                "http://127.0.0.1:8419/sessions",
                json={"project_dir": os.getcwd(), "scene_path": scene_path},
                timeout=5.0,
            )
            if resp.status_code == 201:
                return resp.json()
        except (httpx.ConnectError, httpx.TimeoutException):
            pass
        except Exception as e:
            _log(f"[MCP Bridge] Hub error: {e}")
        return None

    def _cleanup_hub_session(self) -> None:
        if self._hub_session_id is None:
            return


        try:
            httpx.delete(
                f"http://127.0.0.1:8419/sessions/{self._hub_session_id}",
                timeout=3.0,
            )
            _log(f"[MCP Bridge] Cleaned up hub session {self._hub_session_id}")
        except Exception:
            pass
        self._hub_session_id = None

    def _health_check(self) -> bool:
        try:
            resp = httpx.get(f"{self._base_url}/health", timeout=2.0)
            return resp.status_code == 200
        except Exception:
            return False

    def _start_subprocess(self, path: str) -> None:
        port = find_free_port()
        self._subprocess_port = port
        env = build_engine_env(port=port)

        display = env.get("DISPLAY", "")
        wayland = env.get("WAYLAND_DISPLAY", "")
        _log(f"[MCP Bridge] Display env: DISPLAY={display!r} WAYLAND_DISPLAY={wayland!r}")
        _log(f"[MCP Bridge] Control port: {port}")

        self._stderr_lines = []

        self._subprocess = subprocess.Popen(
            [sys.executable, "-m", "pybevy", "dev", path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
        )

        self._stderr_thread = threading.Thread(target=self._read_subprocess_stderr, daemon=True)
        self._stderr_thread.start()

        _log(f"[MCP Bridge] Started subprocess for {path} (pid={self._subprocess.pid})")

        time.sleep(1.0)
        if self._subprocess.poll() is not None:
            exit_code = self._subprocess.returncode
            stderr_output = self._check_stderr_for_errors() or self._get_recent_stderr()
            msg = f"Subprocess exited immediately (code {exit_code})"
            if stderr_output:
                msg += f"\n\nStderr:\n{stderr_output}"
            _log(f"[MCP Bridge] {msg}")
            self._subprocess = None
            self._subprocess_port = None

    def _stop_subprocess(self) -> None:
        if self._subprocess is not None:
            pid = self._subprocess.pid
            try:
                self._subprocess.terminate()
                self._subprocess.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self._subprocess.kill()
                self._subprocess.wait(timeout=2)
            except Exception:
                pass

            self._subprocess = None
            self._subprocess_port = None
            self._stderr_lines = []
            _log(f"[MCP Bridge] Stopped subprocess (pid={pid})")

    def _read_subprocess_stderr(self) -> None:
        proc = self._subprocess
        if proc is None or proc.stderr is None:
            return

        for raw_line in proc.stderr:
            line = raw_line.decode("utf-8", errors="replace").rstrip()
            _log(f"[Bevy] {line}")
            with self._stderr_lock:
                self._stderr_lines.append(line)
                if len(self._stderr_lines) > 100:
                    self._stderr_lines = self._stderr_lines[-100:]

    def _get_recent_stderr(self, max_lines: int = 20) -> str:
        with self._stderr_lock:
            lines = self._stderr_lines[-max_lines:]
        return "\n".join(lines)

    def _check_stderr_for_errors(self) -> str:
        with self._stderr_lock:
            lines = list(self._stderr_lines)

        blocks: list[list[str]] = []
        current_block: list[str] = []
        in_traceback = False
        in_native_error = False

        for line in lines:
            # Python tracebacks
            if "Traceback (most recent call last)" in line:
                if current_block:
                    blocks.append(current_block)
                in_traceback = True
                in_native_error = False
                current_block = [line]
            elif in_traceback:
                current_block.append(line)
                if not line.startswith(" ") and "Error" in line:
                    in_traceback = False
                    blocks.append(current_block)
                    current_block = []
            # Rust panics: thread 'main' panicked at ...
            elif "panicked at" in line and "thread" in line:
                if current_block and in_native_error:
                    blocks.append(current_block)
                in_native_error = True
                in_traceback = False
                current_block = [line]
            # Native/library errors on non-indented lines
            elif not in_traceback and not line.startswith(" ") and (
                # Case-insensitive "error:" on non-indented line
                "error:" in line.lower()
                # tracing/log ERROR lines (e.g. "ERROR bevy_render::renderer:")
                or _is_log_error_line(line)
            ):
                if current_block and in_native_error:
                    blocks.append(current_block)
                in_native_error = True
                current_block = [line]
            # Continuation lines for native error blocks
            elif in_native_error and line.startswith(
                (" ", "\t", "Caused by:", "note:")
            ):
                current_block.append(line)
            elif in_native_error:
                # Non-continuation, non-error line ends the block
                blocks.append(current_block)
                current_block = []
                in_native_error = False

        if current_block:
            blocks.append(current_block)

        return "\n\n".join("\n".join(b) for b in blocks)

    def _success(self, req_id: JsonId, result: object) -> JsonDict:
        return {"jsonrpc": "2.0", "id": req_id, "result": result}

    def _error(self, req_id: JsonId, code: int, message: str) -> JsonDict:
        return {"jsonrpc": "2.0", "id": req_id, "error": {"code": code, "message": message}}

    def _write_response(self, response: JsonDict) -> None:
        line = json.dumps(response, separators=(",", ":"))
        sys.stdout.write(line + "\n")
        sys.stdout.flush()
