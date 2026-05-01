"""MCP tool, resource, and prompt definitions.

Engine tool definitions come from Rust (pybevy_control.tools::list_tools).
Bridge-local tools and resources/prompts are defined here.
"""

from __future__ import annotations

from typing import Any

type JsonDict = dict[str, Any]


def _rust_tools() -> list[JsonDict]:
    """Load engine tool definitions from the Rust pybevy_control crate."""
    try:
        from pybevy._pybevy import mcp as rust_mcp  # type: ignore[import-not-found]  # noqa: PLC0415

        return rust_mcp.rust_tool_definitions()  # type: ignore[no-any-return]
    except Exception:
        return []


def bridge_local_tools() -> list[JsonDict]:
    """Bridge-local tool definitions (not in Rust, handled by Python bridge)."""
    return [
        {
            "name": "get_started",
            "description": (
                "Get PyBevy workflow instructions and scene conventions. "
                "MUST be called before any other tool. Returns essential rules for "
                "scene structure, API usage, and the iterative development workflow. "
                "If you already read the instructions from the initialize response, "
                "pass confirmation_key from the instructions to skip duplicate content."
            ),
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "confirmation_key": {
                        "type": "string",
                        "description": "Key from instructions to confirm you've read them",
                    },
                },
            },
        },
        {
            "name": "search_api",
            "description": (
                "Search across .pyi stub files for type/function names. Returns matching lines (not structured). "
                "Use for discovery when you don't know the class name. "
                "If you already know the type name, use get_type_definition instead."
            ),
            "feature_gate": "api_discovery",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query (grep-like pattern)"},
                },
                "required": ["query"],
            },
        },
        {
            "name": "get_type_definition",
            "description": "Get the full class definition (constructor, methods, fields) from Python stubs.",
            "feature_gate": "api_discovery",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "type_name": {"type": "string", "description": "Class name to look up"},
                },
                "required": ["type_name"],
            },
        },
        {
            "name": "get_schedule_result",
            "description": "Poll status/results of an async schedule. Returns status (running/completed/error/cancelled) and partial or complete results.",
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "schedule_id": {"type": "string", "description": "Schedule ID returned by schedule tool in async mode"},
                },
                "required": ["schedule_id"],
            },
        },
    ]


def builtin_tools() -> list[JsonDict]:
    """All MCP tool definitions: bridge-local tools first (get_started must be first), then Rust engine tools."""
    return bridge_local_tools() + _rust_tools()


def builtin_resources() -> list[JsonDict]:
    """Built-in MCP resource definitions."""
    return [
        {
            "uri": "guide://index",
            "name": "Guide Index",
            "description": "List of available guides with names and descriptions",
            "mimeType": "application/json",
            "feature_gate": None,
        },
        {
            "uri": "api://index",
            "name": "API Index",
            "description": "Module names with class/function lists (lightweight, no content)",
            "mimeType": "application/json",
            "feature_gate": "api_discovery",
        },
        {
            "uri": "scene://entities",
            "name": "Entity List",
            "description": "All entities with their component types and Names",
            "mimeType": "application/json",
            "feature_gate": None,
        },
        {
            "uri": "scene://resources",
            "name": "Resource List",
            "description": "All resources and their values",
            "mimeType": "application/json",
            "feature_gate": None,
        },
        {
            "uri": "scene://systems",
            "name": "System List",
            "description": "Registered systems by stage",
            "mimeType": "application/json",
            "feature_gate": None,
        },
        {
            "uri": "scene://debug",
            "name": "Debug Info",
            "description": "FPS, CPU, GPU, RAM, VRAM, entity/asset counts, system profiling",
            "mimeType": "application/json",
            "feature_gate": None,
        },
    ]


def builtin_prompts(instructions: str) -> list[JsonDict]:
    """Built-in MCP prompt definitions."""
    return [
        {
            "name": "setup_assistant",
            "description": "Onboarding prompt for AI agents working with PyBevy scenes",
            "content": instructions,
        },
    ]


def filtered_resources(api_discovery: bool = True) -> list[JsonDict]:
    """Filter resources by feature gates."""
    result = []
    for res in builtin_resources():
        gate = res.get("feature_gate")
        if gate is None or (gate == "api_discovery" and api_discovery):
            result.append(res)
    return result
