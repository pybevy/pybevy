"""MCP tool, resource, and prompt definitions.

Single source of truth for all MCP protocol definitions.
Feature gates control which tools are exposed based on engine config.
"""

from __future__ import annotations

from typing import Any

type JsonDict = dict[str, Any]


def builtin_tools() -> list[JsonDict]:
    """All built-in MCP tool definitions with feature gates."""
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
            "name": "get_component_schema",
            "description": "Get component field names, types, defaults, and a JSON spawn example",
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Component name (e.g. 'Transform')"},
                },
                "required": ["name"],
            },
        },
        {
            "name": "get_component",
            "description": "Get live field values for a specific component on an entity. Returns structured field data for debugging.",
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity_id": {"type": "integer", "description": "Entity ID (from query_entities)"},
                    "name": {"type": "string", "description": "Entity name (alternative to entity_id)"},
                    "component": {"type": "string", "description": "Component name (e.g. 'Transform', 'PointLight')"},
                },
                "required": ["component"],
            },
        },
        {
            "name": "query_entities",
            "description": "Query entities by With/Without component filters. Also supports Name lookup.",
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "with": {"type": "array", "items": {"type": "string"}, "description": "Component names entities must have"},
                    "without": {"type": "array", "items": {"type": "string"}, "description": "Component names entities must not have"},
                },
            },
        },
        {
            "name": "capture_screenshot",
            "description": "Capture a screenshot. Default 768px wide. UI elements are hidden by default. Use position/look_at to capture from an arbitrary viewpoint without affecting the scene camera. Use gizmos=true to overlay entity ID/Name labels.",
            "feature_gate": "screenshot",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "delay_frames": {"type": "integer", "description": "Frames to wait before capture (default 2)", "default": 2},
                    "max_width": {"type": "integer", "description": "Max image width in pixels (default 768). Use 1280 for detail.", "default": 768},
                    "position": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Camera position [x, y, z]. If set, spawns a temporary debug camera instead of using the scene camera."},
                    "look_at": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Point the camera looks at [x, y, z]. Defaults to [0, 0, 0] if position is set."},
                    "hide_ui": {"type": "boolean", "description": "Hide authored UI Node entities during capture (default true). Internal overlays (hot reload status) are always hidden regardless.", "default": True},
                    "gizmos": {"type": "boolean", "description": "Overlay entity ID/Name labels on the screenshot (default false)", "default": False},
                },
            },
        },
        {
            "name": "capture_timeline",
            "description": "Capture multiple frames over time into a contact sheet. Shows animation/motion in one image.",
            "feature_gate": "screenshot",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "total_frames": {"type": "integer", "description": "Frame span to capture over (~1s at 60fps)", "default": 60},
                    "capture_count": {"type": "integer", "description": "Number of captures (max 20)", "default": 6},
                    "max_width": {"type": "integer", "description": "Max composite width in pixels", "default": 1200},
                    "columns": {"type": "integer", "description": "Grid columns", "default": 3},
                    "position": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Debug camera position [x, y, z]"},
                    "look_at": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Point the camera looks at [x, y, z]. Defaults to [0, 0, 0] if position is set."},
                },
            },
        },
        {
            "name": "reload",
            "description": "Hot-reload the scene (full or partial). Waits for reload to complete before responding — response includes any errors or escalation info. Use 'pause' and 'time_scale' to control time atomically with the reload.",
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mode": {"type": "string", "enum": ["full", "partial"], "description": "Reload mode", "default": "full"},
                    "pause": {"type": "boolean", "description": "Pause time immediately (before reload runs). Scene starts frozen so you can capture at t=0.", "default": False},
                    "time_scale": {"type": "number", "description": "Set time scale atomically with reload (e.g. 0.1 for slow-motion). Applied before any frames run."},
                },
            },
        },
        {
            "name": "get_reload_status",
            "description": "Get current reload generation, mode, and enabled state",
            "feature_gate": None,
            "inputSchema": {"type": "object", "properties": {}},
        },
        {
            "name": "get_last_error",
            "description": "Get the last Python system error traceback",
            "feature_gate": None,
            "inputSchema": {"type": "object", "properties": {}},
        },
        {
            "name": "spawn_entity",
            "description": "Spawn a new entity with components specified as JSON",
            "feature_gate": "manipulation",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "components": {"type": "object", "description": 'Component name -> field values (e.g. {"Transform": {"translation": [0, 5, 0]}})'},
                },
                "required": ["components"],
            },
        },
        {
            "name": "despawn_entity",
            "description": "Remove an entity by ID or Name",
            "feature_gate": "manipulation",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity_id": {"type": "integer", "description": "Entity ID"},
                    "name": {"type": "string", "description": "Entity Name (alternative to entity_id)"},
                },
            },
        },
        {
            "name": "set_component",
            "description": "Update specific fields on a component without replacing it",
            "feature_gate": "manipulation",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity_id": {"type": "integer", "description": "Entity ID"},
                    "name": {"type": "string", "description": "Entity Name (alternative to entity_id)"},
                    "component": {"type": "string", "description": "Component name"},
                    "fields": {"type": "object", "description": "Fields to update"},
                },
                "required": ["component", "fields"],
            },
        },
        {
            "name": "remove_component",
            "description": "Remove a component from an entity",
            "feature_gate": "manipulation",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity_id": {"type": "integer", "description": "Entity ID"},
                    "name": {"type": "string", "description": "Entity Name (alternative to entity_id)"},
                    "component": {"type": "string", "description": "Component to remove"},
                },
                "required": ["component"],
            },
        },
        {
            "name": "set_resource",
            "description": "Insert or update a resource on a running scene (in scene code, use commands.insert_resource() instead)",
            "feature_gate": "manipulation",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "resource_type": {"type": "string", "description": "Resource type name"},
                    "value": {"type": "object", "description": "Resource value as JSON"},
                },
                "required": ["resource_type", "value"],
            },
        },
        {
            "name": "remove_resource",
            "description": "Remove a resource from the world",
            "feature_gate": "manipulation",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "resource_type": {"type": "string", "description": "Resource type name"},
                },
                "required": ["resource_type"],
            },
        },
        {
            "name": "batch",
            "description": "Execute multiple mutation operations in a single round-trip. Each operation runs independently — failures don't abort the batch. Actions: set_component, spawn, despawn, remove_component.",
            "feature_gate": "manipulation",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "operations": {
                        "type": "array",
                        "description": "Array of operations to execute",
                        "items": {
                            "type": "object",
                            "properties": {
                                "action": {"type": "string", "enum": ["set_component", "spawn", "despawn", "remove_component"], "description": "Operation type"},
                                "entity_id": {"type": "integer", "description": "Entity ID (for set_component, despawn, remove_component)"},
                                "name": {"type": "string", "description": "Entity Name (alternative to entity_id)"},
                                "component": {"type": "string", "description": "Component name (for set_component, remove_component)"},
                                "fields": {"type": "object", "description": "Fields to update (for set_component)"},
                                "components": {"type": "object", "description": "Components to spawn (for spawn)"},
                            },
                            "required": ["action"],
                        },
                    },
                },
                "required": ["operations"],
            },
        },
        {
            "name": "set_asset",
            "description": "Update asset properties (material color, mesh settings) live without code reload.",
            "feature_gate": "manipulation",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity_id": {"type": "integer", "description": "Entity ID"},
                    "name": {"type": "string", "description": "Entity Name (alternative to entity_id)"},
                    "component": {"type": "string", "description": "Handle component: MeshMaterial3d, Mesh3d, MeshMaterial2d, AudioPlayer"},
                    "asset_type": {"type": "string", "description": "Asset type: StandardMaterial, Mesh, ColorMaterial, AudioSource"},
                    "fields": {"type": "object", "description": "Fields to update on the asset"},
                },
                "required": ["component", "asset_type", "fields"],
            },
        },
        {
            "name": "get_bounding_box",
            "description": "Get the axis-aligned bounding box (AABB) of an entity, both local and world-space. Requires entity to have a mesh.",
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity_id": {"type": "integer", "description": "Entity ID"},
                    "name": {"type": "string", "description": "Entity Name (alternative to entity_id)"},
                },
            },
        },
        {
            "name": "get_scene_summary",
            "description": "Get a grouped entity inventory: counts by type (e.g. '30 Bubble, 6 Fish, 1 Camera3d'). Faster than query_entities for understanding scene composition.",
            "feature_gate": None,
            "inputSchema": {"type": "object", "properties": {}},
        },
        {
            "name": "query_spatial",
            "description": "Spatial queries between entities. Two modes: (1) Pairwise: provide name_a/name_b to get distance, direction, AABB overlap between two entities. (2) Neighborhood: provide name + radius to find all entities within radius.",
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity_id_a": {"type": "integer", "description": "Entity A ID (pairwise mode)"},
                    "name_a": {"type": "string", "description": "Entity A name (pairwise mode)"},
                    "entity_id_b": {"type": "integer", "description": "Entity B ID (pairwise mode)"},
                    "name_b": {"type": "string", "description": "Entity B name (pairwise mode)"},
                    "entity_id": {"type": "integer", "description": "Center entity ID (neighborhood mode)"},
                    "name": {"type": "string", "description": "Center entity name (neighborhood mode)"},
                    "radius": {"type": "number", "description": "Search radius (neighborhood mode). If set, uses neighborhood mode."},
                    "max_results": {"type": "integer", "description": "Max neighbors to return (default 50)"},
                },
            },
        },
        {
            "name": "check_overlaps",
            "description": "Detect AABB overlaps. Two modes: (1) Single entity: provide entity_id/name to check one entity against all others. (2) Scene-wide: omit entity to find all overlapping pairs. Also detects floating entities (no surface below). Note: AABB overlap detection does not detect ground penetration (models sunk below ground). Use ground_y parameter to detect sunken entities.",
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity_id": {"type": "integer", "description": "Entity ID (single entity mode)"},
                    "name": {"type": "string", "description": "Entity name (single entity mode)"},
                    "include_siblings": {"type": "boolean", "description": "Include siblings under same parent (default false, since parented parts overlap by design)", "default": False},
                    "min_penetration": {"type": "number", "description": "Minimum overlap depth to report (scene-wide mode, default 0.001)", "default": 0.001},
                    "max_results": {"type": "integer", "description": "Max overlapping pairs to return (scene-wide mode, default 100)", "default": 100},
                    "max_float_gap": {"type": "number", "description": "Max gap (units) between entity bottom and surface below to still count as grounded (default 0.1). Increase for scenes with small placement gaps.", "default": 0.1},
                    "ground_y": {"type": "number", "description": "Ground plane Y coordinate for sunk-detection. When provided, entities whose world AABB min_y is below this value are flagged as sunken. Useful for detecting GLB models placed at origin that are half-buried below the ground plane."},
                },
            },
        },
        {
            "name": "reload_and_capture",
            "description": "Primary iteration tool. Reload and capture in one round-trip. Hot-reloads the scene, waits for completion, checks for errors, then captures a screenshot. Returns combined result with reload status + error info + screenshot image.",
            "feature_gate": "screenshot",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mode": {"type": "string", "enum": ["full", "partial"], "description": "Reload mode (default full)", "default": "full"},
                    "pause": {"type": "boolean", "description": "Pause time before reload", "default": False},
                    "time_scale": {"type": "number", "description": "Set time scale atomically with reload"},
                    "delay_frames": {"type": "integer", "description": "Frames to wait after reload before capture (default 30)", "default": 30},
                    "max_width": {"type": "integer", "description": "Max screenshot width (default 768)", "default": 768},
                    "position": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Debug camera position [x, y, z]"},
                    "look_at": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Camera look-at point [x, y, z]"},
                    "hide_ui": {"type": "boolean", "description": "Hide UI during capture (default true)", "default": True},
                },
            },
        },
        {
            "name": "capture_turnaround",
            "description": "Capture multiple viewpoints orbiting around a target, composited into one contact sheet. Auto-fits distance to scene bounds if not specified.",
            "feature_gate": "screenshot",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "look_at": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Center point to orbit around. Auto-detected from scene bounds if omitted."},
                    "distance": {"type": "number", "description": "Camera distance from center. Auto-fitted to scene bounds if omitted."},
                    "elevation": {"type": "number", "description": "Camera elevation in degrees (default 25)", "default": 25},
                    "view_count": {"type": "integer", "description": "Number of orbit positions (default 6)", "default": 6},
                    "include_top": {"type": "boolean", "description": "Include top-down view (default true)", "default": True},
                    "columns": {"type": "integer", "description": "Grid columns in contact sheet (default 3)", "default": 3},
                    "max_width": {"type": "integer", "description": "Max composite width (default 1200)", "default": 1200},
                    "hide_ui": {"type": "boolean", "description": "Hide UI during capture (default true)", "default": True},
                },
            },
        },
        {
            "name": "capture_depth",
            "description": "Capture RGB screenshot + ray-AABB depth samples. Casts rays from camera through sample points (or auto-grid) against entity bounding boxes. Returns structured depth data with hit entity, distance, and world position.",
            "feature_gate": "screenshot",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "position": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Camera position [x, y, z]"},
                    "look_at": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Camera look-at [x, y, z]"},
                    "sample_points": {"type": "array", "items": {"type": "array", "items": {"type": "integer"}, "minItems": 2, "maxItems": 2}, "description": "Screen-space sample points [[x, y], ...]. Auto-generates grid if omitted."},
                    "grid_density": {"type": "integer", "description": "Auto-generate NxN sample grid (default 8 if no sample_points)", "default": 8},
                    "include_rgb": {"type": "boolean", "description": "Include RGB screenshot (default true)", "default": True},
                    "delay_frames": {"type": "integer", "description": "Frames to wait before capture (default 2)", "default": 2},
                    "hide_ui": {"type": "boolean", "description": "Hide UI during capture (default true)", "default": True},
                    "max_width": {"type": "integer", "description": "Max screenshot width (default 768)", "default": 768},
                },
            },
        },
        {
            "name": "run_code",
            "description": "Run arbitrary Python code with World context. Returns stdout/stderr capture and change summary.",
            "feature_gate": "execute_python",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": {"type": "string", "description": "Python code to execute"},
                },
                "required": ["code"],
            },
        },
        {
            "name": "get_registry",
            "description": "Show bridge registry state, entity count, and component detection status",
            "feature_gate": None,
            "inputSchema": {"type": "object", "properties": {}},
        },
        {
            "name": "pause_time",
            "description": "Pause game time. Scene freezes but rendering continues. Useful for inspecting a specific moment. Note: extended pause with a minimized window may cause tool timeouts due to reduced frame processing.",
            "feature_gate": None,
            "inputSchema": {"type": "object", "properties": {}},
        },
        {
            "name": "resume_time",
            "description": "Resume game time after pause.",
            "feature_gate": None,
            "inputSchema": {"type": "object", "properties": {}},
        },
        {
            "name": "set_time_scale",
            "description": "Set game speed multiplier (0.1 = slow-mo, 2.0 = 2x speed). Does not affect pause state.",
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scale": {"type": "number", "description": "Speed multiplier (default 1.0)", "default": 1.0},
                },
            },
        },
        {
            "name": "get_time_status",
            "description": "Get current time state: paused, speed, elapsed seconds.",
            "feature_gate": None,
            "inputSchema": {"type": "object", "properties": {}},
        },
        {
            "name": "seek_time",
            "description": "Jump virtual time to a specific moment. Seeking backwards resets virtual time (preserves speed). Pauses by default so you can capture at that exact time.",
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "seconds": {"type": "number", "description": "Target elapsed time in seconds"},
                    "pause": {"type": "boolean", "description": "Pause after seeking (default true)", "default": True},
                },
                "required": ["seconds"],
            },
        },
        {
            "name": "get_performance",
            "description": "Get full diagnostics: FPS, CPU/GPU/RAM/VRAM usage, entity/asset counts, system profiling times",
            "feature_gate": None,
            "inputSchema": {"type": "object", "properties": {}},
        },
        {
            "name": "register_tool",
            "description": "Register a custom tool from Python (namespaced as custom.*)",
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Tool name (will be prefixed with 'custom.')"},
                    "description": {"type": "string", "description": "Tool description"},
                    "input_schema": {"type": "object", "description": "JSON Schema for tool input"},
                    "python_function": {"type": "string", "description": "Python function name or code"},
                },
                "required": ["name", "description", "python_function"],
            },
        },
        {
            "name": "schedule_actions",
            "description": (
                "Submit batched, timed tool calls that execute inside the engine frame loop "
                "with frame-precise timing. Same-'at' actions fire in the same frame (atomic). "
                "Supports: time control, mutations, queries, screenshots. "
                "NOT supported: get_logs, search_api, run_scene (bridge-local tools)."
            ),
            "feature_gate": None,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "actions": {
                        "type": "array",
                        "description": "Ordered list of tool calls to execute",
                        "items": {
                            "type": "object",
                            "properties": {
                                "tool": {"type": "string", "description": "Tool name to call"},
                                "args": {"type": "object", "description": "Tool arguments (default {})", "default": {}},
                                "at": {"type": "number", "description": "Game-time offset in seconds from schedule start (default 0). Must be monotonically non-decreasing."},
                                "at_frame": {"type": "integer", "description": "Frame offset from schedule start (alternative to 'at'). Cannot mix with 'at'."},
                                "label": {"type": "string", "description": "Label for this action (used by skip_if_error)"},
                                "skip_if_error": {"type": "string", "description": "Skip this action if the action with the given label errored"},
                            },
                            "required": ["tool"],
                        },
                        "minItems": 1,
                        "maxItems": 256,
                    },
                    "mode": {"type": "string", "enum": ["sync", "async"], "description": "sync (default): block until all actions complete. async: return schedule_id immediately.", "default": "sync"},
                    "stop_on_error": {"type": "boolean", "description": "Abort remaining actions on first error (default false)", "default": False},
                },
                "required": ["actions"],
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
