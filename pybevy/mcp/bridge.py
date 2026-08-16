"""MCP stdio bridge for AI integration.

Two-mode dispatch:
  Mode A (no engine): local tools only (get_started, get_guide, run_scene,
  search_api, get_type_definition)
  Mode B (engine running): forward via HTTP REST to ControlPlugin, inject bridge-only tools
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import threading
import time
from importlib.metadata import PackageNotFoundError, version
from typing import Any, TypeVar

from .._pybevy import mcp as _rust_mcp  # type: ignore[import-not-found]
from . import ApiIndex
from .definitions import (
    builtin_prompts,
    builtin_tools,
    filtered_resources,
)
from .engine import (
    build_engine_env,
    find_free_port,
)
from .recorder import SessionRecorder

type JsonId = int | str | None
type JsonDict = dict[str, Any]

_ControlConnectError = _rust_mcp._ControlConnectError
_ControlHttpStatusError = _rust_mcp._ControlHttpStatusError
_ControlTimeoutError = _rust_mcp._ControlTimeoutError
_control_health = _rust_mcp._control_health
_control_last_error = _rust_mcp._control_last_error
_control_request_scene_resource = _rust_mcp._control_request_scene_resource
_control_request_tool = _rust_mcp._control_request_tool
_CapturedLine = TypeVar("_CapturedLine")


def _server_version() -> str:
    """Installed pybevy version, reported to MCP clients in `serverInfo`."""
    try:
        return version("pybevy")
    except PackageNotFoundError:
        return "unknown"


def _log(msg: str) -> None:
    sys.stderr.write(msg + "\n")
    sys.stderr.flush()


# A native error line has "error:" at the start or after a short tool prefix
# ("error: ...", "Error: ...", "wgpu error: ...", "shader compilation error: ...").
# Mid-sentence mentions ("... returned error: No such file or directory" from
# alsa-lib device probing and similar C-library noise) are not errors themselves.
_NATIVE_ERROR_LINE = re.compile(r"(\S+ ){0,3}error:", re.IGNORECASE)

# Bevy colours its tracing output even when stderr is a pipe, so captured lines
# arrive as "\x1b[2m<ts>\x1b[0m \x1b[31mERROR\x1b[0m \x1b[2m<target>...".
# Classify on the stripped text; the raw line is what gets returned.
_ANSI_ESCAPE = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


def _strip_ansi(line: str) -> str:
    return _ANSI_ESCAPE.sub("", line)


def _is_log_error_line(line: str) -> bool:
    """Check if line is a tracing/log ERROR line (e.g. 'ERROR bevy_render::...')."""
    # Strip optional timestamp prefix like "2024-01-01T12:00:00.000Z "
    stripped = _strip_ansi(line).lstrip()
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
        "with hot-reload. For ordinary code changes, call this once, then edit "
        "the .py file and use reload or reload_and_capture. Call it again when "
        "switching scenes, adding or removing bridge-backed plugins, changing "
        "core plugin composition, or requiring a clean restart."
    ),
    "inputSchema": {
        "type": "object",
        "additionalProperties": False,
        "properties": {
            "path": {
                "type": "string",
                "description": "Path to the Python scene file (e.g. 'scenes/my_scene.py')",
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

_MAX_CAPTURED_OUTPUT_LINES = 100


GET_LOGS_TOOL: JsonDict = {
    "name": "get_logs",
    "description": (
        "Get recent captured Bevy subprocess stdout and stderr. With errors_only=true, "
        "combine the live Python system error with matching stderr errors. "
        "Use get_last_error as the primary Python check after reload."
    ),
    "inputSchema": {
        "type": "object",
        "additionalProperties": False,
        "properties": {
            "lines": {
                "type": "integer",
                "description": "Number of recent combined output lines to return (default 50, max 100)",
                "default": 50,
                "minimum": 1,
                "maximum": _MAX_CAPTURED_OUTPUT_LINES,
            },
            "errors_only": {
                "type": "boolean",
                "description": "Only return Python errors/tracebacks (default false)",
                "default": False,
            },
        },
    },
}


_LONG_TIMEOUT_TOOLS = {
    "capture_screenshot",
    "capture_timeline",
    "capture_turnaround",
    "reload_and_capture",
    "capture_depth",
    "capture_stats",
    "schedule_actions",
}

_SCREENSHOT_TOOLS = {
    "capture_screenshot",
    "capture_timeline",
    "capture_turnaround",
    "capture_depth",
    "reload_and_capture",
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
        self._scene_path = os.path.abspath(scene_path) if scene_path else None
        self._scene_display_path = scene_path
        self._protocol_version = protocol_version
        self._recorder: SessionRecorder | None = SessionRecorder() if record else None
        self._subprocess: subprocess.Popen[bytes] | None = None
        self._subprocess_port: int | None = None
        self._stderr_lines: list[str] = []
        self._stderr_repeat_counts: list[int] = []
        self._output_lines: list[tuple[str, str]] = []
        self._output_repeat_counts: list[int] = []
        self._output_lock = threading.Lock()
        self._stdout_thread: threading.Thread | None = None
        self._stderr_thread: threading.Thread | None = None
        self._reaper_thread: threading.Thread | None = None
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
        if self._subprocess_port is not None:
            return self._subprocess_port
        raise RuntimeError("No owned scene control port")

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
                        {
                            "jsonrpc": "2.0",
                            "id": None,
                            "error": {"code": -32700, "message": f"Parse error: {e}"},
                        }
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

    def _has_engine(self) -> bool:
        return self._subprocess is not None and self._subprocess.poll() is None

    def _no_engine_message(self) -> str:
        """Explain why scene tools are unavailable, distinguishing a scene that
        was never started from a subprocess that has since exited."""
        proc = self._subprocess
        if proc is not None and proc.poll() is not None:
            return (
                f"Scene subprocess exited (exit code {proc.returncode}) - e.g. window "
                "closed or crash. Call run_scene again to restart it; get_logs shows "
                "its final output."
            )
        return "No scene loaded. Use the 'run_scene' tool first to start a Bevy app."

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

    def _dispatch_mode_a(
        self, method: str, req_id: JsonId, params: JsonDict
    ) -> JsonDict | None:
        """Mode A: no engine. Only bridge-local tools."""
        if method == "tools/list":
            return self._handle_tools_list_local(req_id)
        if method == "tools/call":
            return self._handle_tools_call_local(req_id, params)
        if method == "resources/list":
            return self._handle_resources_list_local(req_id)
        if method == "resources/templates/list":
            return self._handle_resource_templates_list(req_id)
        if method == "resources/read":
            return self._handle_resources_read_local(req_id, params)
        if method == "prompts/list":
            return self._handle_prompts_list(req_id)
        if method == "prompts/get":
            return self._handle_prompts_get(req_id, params)
        return self._error(req_id, -32601, f"Method not found: {method}")

    def _dispatch_mode_b(
        self, method: str, req_id: JsonId, params: JsonDict
    ) -> JsonDict | None:
        """Mode B: engine running. Forward tool calls via HTTP."""
        if method == "tools/list":
            return self._handle_tools_list_full(req_id)
        if method == "tools/call":
            return self._handle_tools_call_forwarded(req_id, params)
        if method == "resources/list":
            return self._handle_resources_list_full(req_id)
        if method == "resources/templates/list":
            return self._handle_resource_templates_list(req_id)
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
                "serverInfo": {"name": "pybevy-mcp", "version": _server_version()},
                "instructions": self._instructions,
            },
        )

    def _handle_tools_list_local(self, req_id: JsonId) -> JsonDict:
        # Expose ALL tools even without engine: some MCP clients (Codex, Gemini)
        # don't support notifications/tools/list_changed, so they'd never see
        # engine tools if we only added them after run_scene.
        return self._handle_tools_list_full(req_id)

    def _handle_tools_call_local(
        self, req_id: JsonId, params: JsonDict
    ) -> JsonDict | None:
        tool_name = str(params.get("name", ""))
        arguments = params.get("arguments") or {}
        argument_error = self._tool_argument_error(tool_name, arguments)
        if argument_error:
            return self._error(req_id, -32602, argument_error)
        assert isinstance(arguments, dict)

        if tool_name == "get_started":
            return self._handle_get_started(req_id, arguments)
        if tool_name == "run_scene":
            return self._handle_run_scene(req_id, arguments)
        if tool_name == "get_logs":
            return self._handle_get_logs(req_id, arguments)
        if tool_name == "get_guide":
            return self._handle_get_guide(req_id, arguments)
        if tool_name == "search_api":
            return self._handle_search_api(req_id, arguments)
        if tool_name == "get_type_definition":
            return self._handle_get_type_definition(req_id, arguments)
        return self._error(req_id, -32603, self._no_engine_message())

    def _handle_resources_list_local(self, req_id: JsonId) -> JsonDict:
        # Expose all resources including scene://. Clients that don't support
        # list_changed would never see them otherwise.
        return self._handle_resources_list_full(req_id)

    def _handle_resources_read_local(
        self, req_id: JsonId, params: JsonDict
    ) -> JsonDict:
        uri = str(params.get("uri", ""))

        if uri == "guide://index" and self._api_index:
            text = self._api_index.get_guide_index()
            return self._success(
                req_id,
                {
                    "contents": [
                        {"uri": uri, "mimeType": "application/json", "text": text}
                    ]
                },
            )

        if uri.startswith("guide://"):
            name = uri[len("guide://") :]
            if self._api_index:
                content = self._api_index.get_guide(name)
                if content:
                    text = json.dumps({"name": name, "content": content}, indent=2)
                    return self._success(
                        req_id,
                        {
                            "contents": [
                                {
                                    "uri": uri,
                                    "mimeType": "application/json",
                                    "text": text,
                                }
                            ]
                        },
                    )

        if uri == "api://index" and self._api_index:
            text = self._api_index.get_index()
            return self._success(
                req_id,
                {
                    "contents": [
                        {"uri": uri, "mimeType": "application/json", "text": text}
                    ]
                },
            )

        if uri.startswith("api://module/") and self._api_index:
            module_name = uri[len("api://module/") :]
            module_content = self._api_index.get_module_content(module_name)
            if module_content:
                text = json.dumps(
                    {"module": module_name, "content": module_content}, indent=2
                )
                return self._success(
                    req_id,
                    {
                        "contents": [
                            {"uri": uri, "mimeType": "application/json", "text": text}
                        ]
                    },
                )

        if uri.startswith("scene://"):
            return self._error(req_id, -32603, self._no_engine_message())

        return self._error(req_id, -32602, f"Unknown resource: {uri}")

    def _handle_prompts_list(self, req_id: JsonId) -> JsonDict:
        prompts = [
            {"name": p["name"], "description": p["description"], "arguments": []}
            for p in self._prompts
        ]
        return self._success(req_id, {"prompts": prompts})

    def _handle_prompts_get(self, req_id: JsonId, params: JsonDict) -> JsonDict:
        name = str(params.get("name", ""))
        for p in self._prompts:
            if p["name"] == name:
                content = str(p.get("content", ""))
                return self._success(
                    req_id,
                    {
                        "messages": [
                            {
                                "role": "user",
                                "content": {"type": "text", "text": content},
                            }
                        ]
                    },
                )
        return self._error(req_id, -32602, f"Unknown prompt: {name}")

    def _handle_prompts_list_full(self, req_id: JsonId) -> JsonDict:
        prompts = [
            {"name": p["name"], "description": p["description"], "arguments": []}
            for p in self._prompts
        ]
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
        resources: list[JsonDict] = []
        for res in filtered_resources(api_discovery=True):
            uri = res.get("uri")
            if not isinstance(uri, str):
                continue
            resources.append(
                {
                    "uri": uri,
                    "name": res.get("name", ""),
                    "description": res.get("description", ""),
                    "mimeType": res.get("mimeType", "text/plain"),
                }
            )

        # guide://index is useful to clients that can follow resource URIs from
        # its contents, but many MCP adapters expose only resources returned by
        # resources/list. Advertise each guide so those adapters can make every
        # guide directly callable as well.
        if self._api_index:
            try:
                guide_index = json.loads(self._api_index.get_guide_index())
            except (TypeError, json.JSONDecodeError):
                guide_index = []

            listed_uris = {resource["uri"] for resource in resources}
            if isinstance(guide_index, list):
                for guide in guide_index:
                    if not isinstance(guide, dict):
                        continue
                    name = guide.get("name")
                    if not isinstance(name, str) or not name:
                        continue
                    uri = f"guide://{name}"
                    if uri in listed_uris:
                        continue
                    title = guide.get("title")
                    description = guide.get("description")
                    fallback_description = (
                        title
                        if isinstance(title, str)
                        else f"Read the {name} PyBevy guide."
                    )
                    resources.append(
                        {
                            "uri": uri,
                            # A stable name gives resource-to-tool adapters names
                            # such as get_guide_mesh and
                            # get_guide_recipes_outdoor.
                            "name": f"Guide {name}",
                            "description": (
                                description
                                if isinstance(description, str) and description
                                else fallback_description
                            ),
                            "mimeType": "application/json",
                        }
                    )
                    listed_uris.add(uri)
        return self._success(req_id, {"resources": resources})

    def _handle_resource_templates_list(self, req_id: JsonId) -> JsonDict:
        templates: list[JsonDict] = []
        for resource in filtered_resources(api_discovery=True):
            uri_template = resource.get("uriTemplate")
            if not isinstance(uri_template, str):
                continue
            templates.append(
                {
                    "uriTemplate": uri_template,
                    "name": resource.get("name", ""),
                    "description": resource.get("description", ""),
                    "mimeType": resource.get("mimeType", "text/plain"),
                }
            )
        return self._success(req_id, {"resourceTemplates": templates})

    def _handle_resources_read_mode_b(
        self, req_id: JsonId, params: JsonDict
    ) -> JsonDict:
        uri = str(params.get("uri", ""))
        if uri.startswith("scene://"):
            return self._forward_scene_resource(req_id, uri)
        return self._handle_resources_read_local(req_id, params)

    def _handle_tools_call_forwarded(
        self, req_id: JsonId, params: JsonDict
    ) -> JsonDict | None:
        tool_name = str(params.get("name", ""))
        arguments = params.get("arguments") or {}
        argument_error = self._tool_argument_error(tool_name, arguments)
        if argument_error:
            return self._error(req_id, -32602, argument_error)
        assert isinstance(arguments, dict)

        if tool_name == "get_started":
            return self._handle_get_started(req_id, arguments)
        if tool_name == "run_scene":
            return self._handle_run_scene(req_id, arguments)
        if tool_name == "get_logs":
            return self._handle_get_logs(req_id, arguments)
        if tool_name == "get_guide":
            return self._handle_get_guide(req_id, arguments)
        if tool_name == "search_api":
            return self._handle_search_api(req_id, arguments)
        if tool_name == "get_type_definition":
            return self._handle_get_type_definition(req_id, arguments)

        if tool_name in ("reload", "reload_and_capture"):
            with self._output_lock:
                self._stderr_lines.clear()
                self._stderr_repeat_counts.clear()
                self._output_lines.clear()
                self._output_repeat_counts.clear()

        if tool_name == "schedule_actions":
            # Validate that no action uses a bridge-local tool
            _bridge_local = {
                "schedule_actions",
                "get_schedule_result",
                "run_scene",
                "get_started",
                "get_logs",
                "get_guide",
                "search_api",
                "get_type_definition",
            }
            for action in arguments.get("actions", []):
                tool = action.get("tool", "")
                if tool in _bridge_local:
                    return self._error(
                        req_id,
                        -32602,
                        f"Tool '{tool}' cannot be used inside a schedule (bridge-local tool)",
                    )
            # Dynamic timeout based on max 'at' value in actions
            max_at = max(
                (a.get("at", 0) for a in arguments.get("actions", [])), default=0
            )
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
        except _ControlTimeoutError:
            hint = ""
            _time_tools = {
                "pause_time",
                "resume_time",
                "set_time_scale",
                "get_time_status",
                "seek_time",
            }
            if tool_name not in _time_tools:
                hint = " Hint: if scene time is paused or the window is minimized, the engine may stop processing requests. Try resume_time or focus the window."
            return self._error(
                req_id,
                -32603,
                f"Engine timeout ({timeout:.0f}s) for {tool_name}.{hint}",
            )
        except _ControlConnectError:
            return self._error(
                req_id, -32603, "Engine not responding. Use 'run_scene' to restart."
            )
        except _ControlHttpStatusError as e:
            return self._error(req_id, -32603, str(e))
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

        Screenshots are returned as MCP image content blocks. Large images use
        bounded inline previews so provider download limits do not discard an
        otherwise successful capture.
        """
        # Schedule results: extract images from action results
        if tool_name in ("schedule_actions", "get_schedule_result"):
            return McpBridge._format_schedule_result(result)

        if tool_name not in _SCREENSHOT_TOOLS:
            return [{"type": "text", "text": json.dumps(result, indent=2)}]

        content: list[JsonDict] = []

        # reload_and_capture: image is in "screenshot" key
        # Other screenshot tools: image is in "image" key
        image_key = (
            "screenshot"
            if tool_name in ("reload_and_capture", "capture_depth")
            else "image"
        )
        image_data = result.get(image_key)
        delivery = result.get("image_delivery")
        mime_type = (
            delivery.get("inline_mime_type", "image/png")
            if isinstance(delivery, dict)
            else "image/png"
        )

        if isinstance(image_data, str) and len(image_data) > 100:
            content.append({"type": "image", "data": image_data, "mimeType": mime_type})

            metadata = {
                k: v
                for k, v in result.items()
                if k not in (image_key, "image_delivery")
            }
            if isinstance(delivery, dict):
                metadata.update(delivery)
            if metadata:
                content.append({"type": "text", "text": json.dumps(metadata, indent=2)})
        elif isinstance(delivery, dict):
            metadata = {
                k: v
                for k, v in result.items()
                if k not in (image_key, "image_delivery")
            }
            metadata.update(delivery)
            content.append({"type": "text", "text": json.dumps(metadata, indent=2)})
        else:
            # No image or small data: return as text
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
                    delivery = inner.get("image_delivery")
                    mime_type = (
                        delivery.get("inline_mime_type", "image/png")
                        if isinstance(delivery, dict)
                        else "image/png"
                    )
                    content.append(
                        {"type": "image", "data": image_data, "mimeType": mime_type}
                    )
                    image_indices.append(action_result.get("index", -1))
                    # Remove large base64 from the JSON to avoid token bloat
                    inner[key] = "<extracted to bounded MCP image block>"
                    break

        # Add the full result JSON (with images replaced by placeholders)
        content.append({"type": "text", "text": json.dumps(result, indent=2)})
        return content

    def _call_rest_api(
        self, tool_name: str, arguments: JsonDict, timeout: float = 30.0
    ) -> JsonDict:
        """Dispatch a typed loopback request through the Rust control client."""

        return _control_request_tool(
            self._engine_port,
            tool_name,
            arguments,
            timeout,
        )

    def _forward_scene_resource(self, req_id: JsonId, uri: str) -> JsonDict:
        """Forward scene:// resource reads via HTTP."""

        try:
            payload = _control_request_scene_resource(
                self._engine_port,
                uri,
                10.0,
            )
            text = json.dumps(payload, indent=2)
            return self._success(
                req_id,
                {
                    "contents": [
                        {"uri": uri, "mimeType": "application/json", "text": text}
                    ]
                },
            )
        except ValueError as e:
            return self._error(req_id, -32602, str(e))
        except Exception as e:
            return self._error(req_id, -32603, f"Failed to read {uri}: {e}")

    def _handle_get_started(
        self, req_id: JsonId, arguments: JsonDict | None = None
    ) -> JsonDict:
        headless_note = (
            "\n\nGraphical scenes are launched directly. If this environment has no "
            "graphical session, use run_scene(path=..., headless=True) with a scene "
            "that disables WinitPlugin and renders to "
            "RenderTarget.Image(ImageRenderTarget(...)). Read guide://headless for "
            "a complete setup."
        )
        key = str((arguments or {}).get("confirmation_key", ""))
        if key == "pybevy-ready":
            text = (
                'Instructions confirmed. Tools unlocked. Call get_guide("patterns") '
                "or read the guide://patterns MCP resource, then run_scene. Never use "
                "filesystem read for guide:// URIs."
            )
            text += headless_note
            return self._success(req_id, {"content": [{"type": "text", "text": text}]})

        instructions = self._instructions
        if not instructions:
            instructions = (
                'No instructions available. Call get_guide("patterns") or read the '
                "guide://patterns MCP resource for scene conventions; never use "
                "filesystem read for guide:// URIs."
            )
        instructions += headless_note
        return self._success(
            req_id, {"content": [{"type": "text", "text": instructions}]}
        )

    def _handle_get_guide(self, req_id: JsonId, arguments: JsonDict) -> JsonDict:
        name = str(arguments.get("name", "")).strip()
        name = name.removeprefix("guide://").strip()
        if not name:
            return self._error(req_id, -32602, "Missing 'name' parameter")
        if not self._api_index:
            return self._error(req_id, -32603, "ApiIndex not available")

        content: str | None
        if name == "index":
            content = self._api_index.get_guide_index()
        else:
            content = self._api_index.get_guide(name)
        if not content:
            return self._error(req_id, -32602, f"Unknown guide: {name}")

        return self._success(
            req_id,
            {"content": [{"type": "text", "text": content}]},
        )

    def _handle_run_scene(self, req_id: JsonId, arguments: JsonDict) -> JsonDict | None:
        display_path = str(arguments.get("path", ""))
        headless = bool(arguments.get("headless", False))
        if not display_path:
            return self._error(req_id, -32602, "Missing 'path' parameter")
        path = os.path.abspath(display_path)
        if not os.path.exists(path):
            return self._error(req_id, -32602, f"File not found: {display_path}")

        self._stop_subprocess()
        self._scene_path = path
        self._scene_display_path = display_path
        self._start_subprocess(path)

        time.sleep(2.0)

        proc = self._subprocess
        if proc is not None and proc.poll() is not None:
            exit_code = proc.returncode
            stderr_output = self._check_stderr_for_errors()
            process_output = self._get_recent_process_output()
            error_msg = f"Bevy subprocess crashed on startup (exit code {exit_code})"
            if stderr_output or process_output:
                error_msg += (
                    f"\n\nSubprocess output:\n{process_output or stderr_output}"
                )
            if not headless and self._looks_like_graphical_startup_failure(
                process_output or stderr_output
            ):
                error_msg += (
                    "\n\nThis looks like a graphical-session startup failure. "
                    "If no display is available, retry with headless=True and read "
                    "guide://headless."
                )
            return self._error(req_id, -32603, error_msg)

        bind_failure = self._control_bind_failure()
        if bind_failure:
            port = self._subprocess_port
            process_output = self._get_recent_process_output()
            self._stop_subprocess()
            error_msg = (
                f"PyBevy control server failed to bind on port {port}; "
                "the scene subprocess was stopped. Retry run_scene to allocate a new port."
            )
            if process_output:
                error_msg += f"\n\nSubprocess output:\n{process_output}"
            else:
                error_msg += f"\n\nSubprocess output:\n{bind_failure}"
            return self._error(req_id, -32603, error_msg)

        status_parts = [
            f"Scene loaded: {display_path}",
            "PyBevy app starting with hot-reload enabled.",
            "Hot-reload is active for ordinary code changes. Restart with run_scene after bridge-backed plugin or core plugin-composition changes.",
            "Workflow: edit the .py file -> reload or reload_and_capture -> verify screenshot -> iterate.",
        ]

        stderr_errors = self._check_stderr_for_errors()
        if stderr_errors:
            status_parts.append(
                f"\nWARNING: Python errors detected during startup:\n{stderr_errors}"
            )

        startup_error, _ = self._get_last_system_error()
        if startup_error:
            self._pending_notifications.append(
                {"jsonrpc": "2.0", "method": "notifications/tools/list_changed"}
            )
            self._pending_notifications.append(
                {"jsonrpc": "2.0", "method": "notifications/resources/list_changed"}
            )
            return self._error(
                req_id,
                -32603,
                f"Scene system failed during initial load: {startup_error}",
            )

        self._pending_notifications.append(
            {"jsonrpc": "2.0", "method": "notifications/tools/list_changed"}
        )
        self._pending_notifications.append(
            {"jsonrpc": "2.0", "method": "notifications/resources/list_changed"}
        )
        return self._success(
            req_id, {"content": [{"type": "text", "text": "\n".join(status_parts)}]}
        )

    def _handle_search_api(self, req_id: JsonId, arguments: JsonDict) -> JsonDict:
        if "query" not in arguments:
            return self._error(req_id, -32602, "Missing 'query' parameter")
        query = str(arguments["query"])
        if not query:
            return self._error(
                req_id, -32602, "Empty 'query': must be at least 1 character"
            )
        if not self._api_index:
            return self._error(req_id, -32603, "ApiIndex not available")

        # Default 50, hard ceiling 200. Reject obviously bad values rather than
        # silently coercing: the caller likely wants to know.
        default_limit = 50
        max_limit = 200
        raw_limit = arguments.get("limit", default_limit)
        try:
            limit = int(raw_limit)
        except (TypeError, ValueError):
            return self._error(req_id, -32602, "'limit' must be an integer")
        if limit < 1:
            return self._error(req_id, -32602, "'limit' must be >= 1")
        if limit > max_limit:
            limit = max_limit

        raw = self._api_index.search(query, limit)
        try:
            payload = json.loads(raw)
            results_json = json.dumps(payload.get("results", []), indent=2)
            total = int(payload.get("total", 0))
            truncated = bool(payload.get("truncated", False))
        except (ValueError, TypeError, AttributeError):
            # Defensive fallback: treat the raw string as the displayed results.
            results_json = raw
            total = 0
            truncated = False

        text = results_json
        if truncated:
            omitted = total - limit
            text += (
                f"\n\n... {omitted} more result(s) omitted (total {total}; "
                f"refine query or pass limit up to {max_limit})."
            )
        if results_json and results_json.strip() not in ("[]", ""):
            text += "\n\nTip: Check guide://index for curated topic guides (faster than API search). Most types available via `from pybevy.prelude import *`. Use get_type_definition('Name') for full class or function definitions."
        return self._success(req_id, {"content": [{"type": "text", "text": text}]})

    def _handle_get_type_definition(
        self, req_id: JsonId, arguments: JsonDict
    ) -> JsonDict:
        type_name = str(arguments.get("type_name", ""))
        if not type_name:
            return self._error(req_id, -32602, "Missing 'type_name' parameter")
        if not self._api_index:
            return self._error(req_id, -32603, "ApiIndex not available")

        structured = self._api_index.get_type_definition_structured(type_name)
        if structured:
            definition = json.loads(structured)
            if definition.get("error") == "ambiguous_type_name":
                result = json.dumps(
                    {
                        "type_name": type_name,
                        "error": "Ambiguous type name; retry with a qualified candidate",
                        "candidates": definition.get("candidates", []),
                    },
                    indent=2,
                )
            else:
                result = json.dumps(
                    {"type_name": type_name, "definition": definition}, indent=2
                )
        else:
            result = json.dumps(
                {"type_name": type_name, "error": "Symbol not found in stubs"}, indent=2
            )

        tip = "Tip: For scene patterns and code templates, read guide://patterns. For topic-specific docs (lighting, materials, camera), check guide://index."
        return self._success(
            req_id,
            {
                "content": [
                    {"type": "text", "text": result},
                    {"type": "text", "text": tip},
                ]
            },
        )

    def _handle_get_logs(self, req_id: JsonId, arguments: JsonDict) -> JsonDict:
        try:
            lines = int(arguments.get("lines", 50))  # type: ignore[call-overload]
        except (TypeError, ValueError):
            return self._error(req_id, -32602, "get_logs 'lines' must be an integer")
        errors_only = bool(arguments.get("errors_only", False))

        if self._subprocess is None:
            return self._error(
                req_id, -32603, "No scene loaded. Use 'run_scene' first."
            )

        if errors_only:
            sections: list[str] = []
            system_error, lookup_error = self._get_last_system_error()
            if system_error:
                sections.append(
                    f"Python system error (get_last_error):\n{system_error}"
                )

            stderr_errors = self._check_stderr_for_errors()
            # The block keeps raw lines; the live error arrives plain.
            stderr_plain = _strip_ansi(stderr_errors)
            if stderr_errors and (
                not system_error
                or (
                    stderr_plain not in system_error
                    and system_error not in stderr_plain
                )
            ):
                sections.append(f"Matching subprocess stderr:\n{stderr_errors}")

            if sections:
                output = "\n\n".join(sections)
            elif lookup_error:
                output = (
                    "No matching errors in captured stderr. The live Python system-error "
                    f"channel could not be checked: {lookup_error}"
                )
            else:
                output = "No Python system errors or matching stderr errors detected."
        else:
            line_limit = max(1, min(lines, _MAX_CAPTURED_OUTPUT_LINES))
            output = self._get_recent_process_output(max_lines=line_limit)
            if not output:
                output = "(no output captured yet)"

        # A dead subprocess still serves its buffered output; say so, otherwise
        # stale logs make the scene look alive.
        if self._subprocess is not None and self._subprocess.poll() is not None:
            output += (
                f"\n\n(note: scene subprocess has exited with code "
                f"{self._subprocess.returncode}; logs above are its final output)"
            )

        return self._success(req_id, {"content": [{"type": "text", "text": output}]})

    def _get_last_system_error(self) -> tuple[str, str | None]:
        """Read the engine's live LastSystemError slot for error-only logs."""
        try:
            payload = _control_last_error(self._engine_port, 2.0)
        except Exception as lookup_exception:
            return "", str(lookup_exception)

        system_error = payload.get("error") if isinstance(payload, dict) else None
        if not system_error:
            return "", None
        traceback = payload.get("traceback")
        if traceback:
            traceback_text = str(traceback).rstrip()
            error_text = str(system_error)
            if error_text not in traceback_text:
                return f"{traceback_text}\n{error_text}", None
            return traceback_text, None
        return str(system_error), None

    def _health_check(self) -> bool:
        try:
            return _control_health(self._engine_port, 2.0)
        except Exception:
            return False

    @staticmethod
    def _looks_like_graphical_startup_failure(output: str) -> bool:
        lowered = output.casefold()
        return any(
            marker in lowered
            for marker in (
                "no display",
                "display server",
                "open display",
                "xopendisplay",
                "event loop",
                "eventloop",
                "wayland",
                "window creation",
                "create window",
                "winit",
                "x11",
            )
        )

    def _control_bind_failure(self) -> str | None:
        """Return the engine's control-listener bind error, if one was logged."""
        marker = "[Control] Failed to bind to "
        with self._output_lock:
            for stream, line in reversed(self._output_lines):
                if stream == "stderr" and marker in line:
                    return line
            for line in reversed(self._stderr_lines):
                if marker in line:
                    return line
        return None

    def _start_subprocess(self, path: str) -> None:
        port = find_free_port()
        self._subprocess_port = port
        env = build_engine_env(port=port)

        display = env.get("DISPLAY", "")
        wayland = env.get("WAYLAND_DISPLAY", "")
        _log(
            f"[MCP Bridge] Display env: DISPLAY={display!r} WAYLAND_DISPLAY={wayland!r}"
        )
        _log(f"[MCP Bridge] Control port: {port}")

        with self._output_lock:
            self._stderr_lines.clear()
            self._stderr_repeat_counts.clear()
            self._output_lines.clear()
            self._output_repeat_counts.clear()

        self._subprocess = subprocess.Popen(
            [sys.executable, "-m", "pybevy", "dev", path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
        )

        self._stdout_thread = threading.Thread(
            target=self._read_subprocess_stdout,
            args=(self._subprocess,),
            daemon=True,
        )
        self._stderr_thread = threading.Thread(
            target=self._read_subprocess_stderr,
            args=(self._subprocess,),
            daemon=True,
        )
        self._reaper_thread = threading.Thread(
            target=self._reap_subprocess,
            args=(self._subprocess,),
            daemon=True,
        )
        self._stdout_thread.start()
        self._stderr_thread.start()
        self._reaper_thread.start()

        _log(f"[MCP Bridge] Started subprocess for {path} (pid={self._subprocess.pid})")

        time.sleep(1.0)
        if self._subprocess.poll() is not None:
            exit_code = self._subprocess.returncode
            stderr_output = self._check_stderr_for_errors()
            process_output = self._get_recent_process_output()
            msg = f"Subprocess exited immediately (code {exit_code})"
            if stderr_output or process_output:
                msg += f"\n\nOutput:\n{process_output or stderr_output}"
            _log(f"[MCP Bridge] {msg}")

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
            with self._output_lock:
                self._stderr_lines.clear()
                self._stderr_repeat_counts.clear()
                self._output_lines.clear()
                self._output_repeat_counts.clear()
            _log(f"[MCP Bridge] Stopped subprocess (pid={pid})")

    def _reap_subprocess(self, proc: subprocess.Popen[bytes]) -> None:
        # Without this the child stays a zombie until some handler happens to poll().
        try:
            exit_code = proc.wait()
        except Exception:
            return
        _log(f"[MCP Bridge] Subprocess exited (pid={proc.pid}, code={exit_code})")

    def _read_subprocess_stdout(
        self,
        proc: subprocess.Popen[bytes] | None = None,
    ) -> None:
        proc = proc or self._subprocess
        stdout = getattr(proc, "stdout", None)
        if stdout is None:
            return

        for raw_line in stdout:
            if self._subprocess is not proc:
                break
            line = raw_line.decode("utf-8", errors="replace").rstrip()
            _log(f"[Bevy stdout] {line}")
            self._append_process_output("stdout", line)

    def _read_subprocess_stderr(
        self,
        proc: subprocess.Popen[bytes] | None = None,
    ) -> None:
        proc = proc or self._subprocess
        if proc is None or proc.stderr is None:
            return

        for raw_line in proc.stderr:
            if self._subprocess is not proc:
                break
            line = raw_line.decode("utf-8", errors="replace").rstrip()
            _log(f"[Bevy stderr] {line}")
            with self._output_lock:
                self._append_captured_line_locked(
                    self._stderr_lines,
                    self._stderr_repeat_counts,
                    line,
                )
                self._append_process_output_locked("stderr", line)

    def _append_process_output(self, stream: str, line: str) -> None:
        with self._output_lock:
            self._append_process_output_locked(stream, line)

    def _append_process_output_locked(self, stream: str, line: str) -> None:
        self._append_captured_line_locked(
            self._output_lines,
            self._output_repeat_counts,
            (stream, line),
        )

    @staticmethod
    def _append_captured_line_locked(
        lines: list[_CapturedLine],
        repeat_counts: list[int],
        line: _CapturedLine,
    ) -> None:
        if len(repeat_counts) != len(lines):
            repeat_counts[:] = [1] * len(lines)
        if lines and lines[-1] == line:
            repeat_counts[-1] += 1
            return
        lines.append(line)
        repeat_counts.append(1)
        if len(lines) > _MAX_CAPTURED_OUTPUT_LINES:
            del lines[:-_MAX_CAPTURED_OUTPUT_LINES]
            del repeat_counts[:-_MAX_CAPTURED_OUTPUT_LINES]

    @staticmethod
    def _format_captured_line(line: str, repeat_count: int) -> str:
        if repeat_count == 1:
            return line
        return f"{line} [repeated: {repeat_count} occurrences]"

    def _get_recent_process_output(self, max_lines: int = 20) -> str:
        with self._output_lock:
            if len(self._output_repeat_counts) != len(self._output_lines):
                self._output_repeat_counts = [1] * len(self._output_lines)
            lines = self._output_lines[-max_lines:]
            repeat_counts = self._output_repeat_counts[-max_lines:]
            # Keep direct stderr injection useful to diagnostics and tests even
            # when no reader populated the combined buffer.
            if not lines and self._stderr_lines:
                lines = [("stderr", line) for line in self._stderr_lines[-max_lines:]]
                if len(self._stderr_repeat_counts) != len(self._stderr_lines):
                    self._stderr_repeat_counts = [1] * len(self._stderr_lines)
                repeat_counts = self._stderr_repeat_counts[-max_lines:]
        return "\n".join(
            f"[{stream}] {self._format_captured_line(line, repeat_count)}"
            for (stream, line), repeat_count in zip(lines, repeat_counts, strict=True)
        )

    def _check_stderr_for_errors(self) -> str:
        with self._output_lock:
            if len(self._stderr_repeat_counts) != len(self._stderr_lines):
                self._stderr_repeat_counts = [1] * len(self._stderr_lines)
            lines = [
                self._format_captured_line(line, repeat_count)
                for line, repeat_count in zip(
                    self._stderr_lines,
                    self._stderr_repeat_counts,
                    strict=True,
                )
            ]

        blocks: list[list[str]] = []
        current_block: list[str] = []
        in_traceback = False
        in_native_error = False

        for line in lines:
            plain = _strip_ansi(line)
            # Python tracebacks
            if "Traceback (most recent call last)" in plain:
                if current_block:
                    blocks.append(current_block)
                in_traceback = True
                in_native_error = False
                current_block = [line]
            elif in_traceback:
                current_block.append(line)
                if not plain.startswith(" ") and "Error" in plain:
                    in_traceback = False
                    blocks.append(current_block)
                    current_block = []
            # Rust panics: thread 'main' panicked at ...
            elif "panicked at" in plain and "thread" in plain:
                if current_block and in_native_error:
                    blocks.append(current_block)
                in_native_error = True
                in_traceback = False
                current_block = [line]
            # Native/library errors on non-indented lines
            elif (
                not in_traceback
                and not plain.startswith(" ")
                and (
                    # Anchored "error:" (optionally after a short tool prefix)
                    _NATIVE_ERROR_LINE.match(plain)
                    # tracing/log ERROR lines (e.g. "ERROR bevy_render::renderer:")
                    or _is_log_error_line(plain)
                )
            ):
                if current_block and in_native_error:
                    blocks.append(current_block)
                in_native_error = True
                current_block = [line]
            # Continuation lines for native error blocks
            elif in_native_error and (
                not plain or plain.startswith((" ", "\t", "Caused by:", "note:"))
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

    def _tool_argument_error(self, tool_name: str, arguments: object) -> str | None:
        if not isinstance(arguments, dict):
            return f"Tool '{tool_name}' arguments must be an object"

        definitions = [*self._tools, LOAD_SCENE_TOOL, GET_LOGS_TOOL]
        tool = next(
            (item for item in definitions if item.get("name") == tool_name), None
        )
        if tool is None:
            return None

        schema = tool.get("inputSchema")
        if not isinstance(schema, dict):
            return None
        properties = schema.get("properties")
        if not isinstance(properties, dict):
            properties = {}

        unknown = sorted(set(arguments) - set(properties))
        if not unknown:
            return None

        accepted = ", ".join(sorted(properties)) or "none"
        return (
            f"Unknown parameter(s) for tool '{tool_name}': {', '.join(unknown)}. "
            f"Accepted parameters: {accepted}"
        )

    def _error(self, req_id: JsonId, code: int, message: str) -> JsonDict:
        return {
            "jsonrpc": "2.0",
            "id": req_id,
            "error": {"code": code, "message": message},
        }

    def _write_response(self, response: JsonDict) -> None:
        line = json.dumps(response, separators=(",", ":"))
        sys.stdout.write(line + "\n")
        sys.stdout.flush()
