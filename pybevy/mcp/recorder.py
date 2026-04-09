"""JSONL session recorder for MCP calls.

When ``--record`` is passed to ``pybevy mcp``, each JSON-RPC request/response
pair is written as a single line to ``calls.jsonl`` inside a per-session
directory under ``.pybevy/logs/``.  Screenshot images are extracted from
responses and saved as numbered PNGs alongside the log file.
"""

from __future__ import annotations

import base64
import json
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

type JsonDict = dict[str, Any]


class SessionRecorder:
    """Records MCP calls to ``.pybevy/logs/<session>/calls.jsonl``."""

    def __init__(self, base_dir: str = ".pybevy/logs") -> None:
        ts = datetime.now(UTC).strftime("%Y-%m-%dT%H-%M-%S")
        self._dir = Path(base_dir) / ts
        self._dir.mkdir(parents=True, exist_ok=True)
        self._log_path = self._dir / "calls.jsonl"
        self._log_file = open(self._log_path, "a")  # noqa: SIM115
        self._image_counter = 0
        sys.stderr.write(f"[MCP Recorder] Session dir: {self._dir}\n")
        sys.stderr.flush()

    @property
    def session_dir(self) -> Path:
        return self._dir

    def record(
        self,
        request: JsonDict,
        response: JsonDict | None,
        duration_ms: float,
    ) -> None:
        """Append one request/response record to the JSONL log."""
        method = str(request.get("method", ""))
        params: JsonDict = request.get("params") or {}

        entry: JsonDict = {
            "ts": datetime.now(UTC).isoformat(),
            "method": method,
            "duration_ms": round(duration_ms, 1),
        }

        if method == "tools/call":
            entry["tool"] = params.get("name")
            args = params.get("arguments")
            if args:
                entry["args"] = args

        # Result / error
        if response is not None:
            err = response.get("error")
            if err:
                entry["error"] = err.get("message", str(err))
            else:
                entry["result"] = self._summarise_result(response.get("result"))

        self._log_file.write(json.dumps(entry, default=str) + "\n")
        self._log_file.flush()

    def _summarise_result(self, result: Any) -> Any:  # noqa: ANN401
        """Return a JSON-safe summary, saving images to disk."""
        if not isinstance(result, dict):
            return result

        content = result.get("content")
        if not isinstance(content, list):
            return result

        blocks: list[JsonDict] = []
        for block in content:
            if block.get("type") == "image" and "data" in block:
                fname = self._save_image(
                    block["data"],
                    block.get("mimeType", "image/png"),
                )
                blocks.append({"type": "image", "file": fname})
            elif block.get("type") == "text":
                text = block.get("text", "")
                try:
                    blocks.append({"type": "text", "data": json.loads(text)})
                except (json.JSONDecodeError, TypeError):
                    blocks.append({"type": "text", "data": text})
            else:
                blocks.append(block)
        return blocks

    def _save_image(self, b64_data: str, mime_type: str) -> str:
        self._image_counter += 1
        ext = "png" if "png" in mime_type else "jpg"
        filename = f"{self._image_counter:03d}.{ext}"
        try:
            (self._dir / filename).write_bytes(base64.b64decode(b64_data))
        except Exception:
            return f"{filename} (save failed)"
        return filename

    def close(self) -> None:
        self._log_file.close()
