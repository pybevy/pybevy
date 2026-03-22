"""JSON Schema generation from dataclass fields for MCP tool registration."""

from __future__ import annotations

import dataclasses
from types import UnionType
from typing import Any, Union, get_args, get_origin, get_type_hints

# Mapping from Python types to JSON Schema types
_TYPE_MAP: dict[type, str] = {
    str: "string",
    int: "integer",
    float: "number",
    bool: "boolean",
}


def _python_type_to_json_schema(tp: type) -> dict[str, Any]:
    """Convert a Python type annotation to a JSON Schema fragment."""
    # Handle basic types
    if tp in _TYPE_MAP:
        return {"type": _TYPE_MAP[tp]}

    origin = get_origin(tp)
    args = get_args(tp)

    # list[X] -> {"type": "array", "items": ...}
    if origin is list:
        if args:
            return {"type": "array", "items": _python_type_to_json_schema(args[0])}
        return {"type": "array"}

    # dict[K, V] -> {"type": "object", "additionalProperties": ...}
    if origin is dict:
        schema: dict[str, Any] = {"type": "object"}
        if len(args) >= 2:
            schema["additionalProperties"] = _python_type_to_json_schema(args[1])
        return schema

    # Optional[X] / X | None -> nullable
    if origin is Union or isinstance(tp, UnionType):
        union_args = get_args(tp)
        non_none = [a for a in union_args if a is not type(None)]
        if len(non_none) == 1:
            inner = _python_type_to_json_schema(non_none[0])
            return {**inner, "nullable": True}

    # tuple[X, Y, ...] -> {"type": "array", "items": [...], "minItems": N, "maxItems": N}
    if origin is tuple:
        if args:
            return {
                "type": "array",
                "items": [_python_type_to_json_schema(a) for a in args],
                "minItems": len(args),
                "maxItems": len(args),
            }
        return {"type": "array"}

    # Fallback
    return {"type": "object"}


def generate_json_schema(cls: type) -> dict[str, Any]:
    """Generate a JSON Schema from a dataclass or class with type annotations.

    Returns a JSON Schema object describing the class fields, suitable
    for MCP tool inputSchema.
    """
    properties: dict[str, Any] = {}
    required: list[str] = []

    # Get type hints
    try:
        hints = get_type_hints(cls)
    except Exception:
        hints = getattr(cls, "__annotations__", {})

    # Get dataclass field defaults if available
    defaults: dict[str, Any] = {}
    if dataclasses.is_dataclass(cls):
        for field in dataclasses.fields(cls):
            if field.default is not dataclasses.MISSING:
                defaults[field.name] = field.default
            elif field.default_factory is not dataclasses.MISSING:
                defaults[field.name] = field.default_factory()

    # Collect field metadata (descriptions) from dataclass fields
    field_meta: dict[str, dict[str, Any]] = {}
    if dataclasses.is_dataclass(cls):
        for f in dataclasses.fields(cls):
            if f.metadata:
                field_meta[f.name] = dict(f.metadata)

    for name, type_hint in hints.items():
        if name.startswith("_"):
            continue

        prop = _python_type_to_json_schema(type_hint)

        # Add default value if present
        if name in defaults:
            prop["default"] = defaults[name]

        # Add field description from metadata if present
        meta = field_meta.get(name)
        if meta and "description" in meta:
            prop["description"] = meta["description"]

        properties[name] = prop

        # Fields without defaults are required
        if name not in defaults:
            required.append(name)

    schema: dict[str, Any] = {
        "type": "object",
        "properties": properties,
    }
    if required:
        schema["required"] = required

    return schema
