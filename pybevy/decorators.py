import dataclasses
import os
import sys
from collections.abc import Callable
from functools import wraps
from typing import TypeVar

from .app import Plugin
from .ecs import Component, Resource

RT = TypeVar("RT", bound=Resource)


def resource(cls: type[RT]) -> type[RT]:
    """Decorator to register a class as a global ECS resource.

    IMPORTANT: The class MUST also inherit from Resource. Both the decorator
    AND the inheritance are required - using only one will cause a runtime error.

    Example:
        @resource
        class GameState(Resource):  # MUST inherit from Resource
            score: int = 0
    """
    if not issubclass(cls, Resource):
        raise TypeError(f"{cls.__name__} must inherit from Resource.")

    # Mark the resource as properly decorated
    # This allows us to detect resources missing the @resource decorator
    cls.__pybevy_resource_decorated__ = True

    return cls


CT = TypeVar("CT", bound=Component)

# Component caching for hot reload support
_component_cache: dict[str, type[Component]] = {}
_component_cache_enabled = False


def clear_component_cache(verbose: bool = False) -> None:
    """Clear the component cache. Called before full reload to allow fresh component definitions."""
    global _component_cache
    if verbose:
        print(f"🗑️  Clearing component cache ({len(_component_cache)} components)")
    _component_cache.clear()
    if verbose:
        print(f"✅ Component cache cleared (now {len(_component_cache)} components)")


def enable_component_caching(enabled: bool, verbose: bool = False) -> None:
    """Enable or disable component caching.

    When enabled (partial reload mode), component classes are cached by qualified name
    to ensure system references remain valid across reloads.

    When disabled (full reload mode), components are freshly registered each time.
    """
    global _component_cache_enabled
    prev_state = _component_cache_enabled
    _component_cache_enabled = enabled
    if verbose:
        print(f"🔧 Component caching: {prev_state} → {enabled}")
    if not enabled:
        # Clear cache when disabling to ensure fresh state
        clear_component_cache(verbose=verbose)


def component(
    cls: type[CT] | None = None,
    *,
    storage: str | None = None,
) -> type[CT] | Callable[[type[CT]], type[CT]]:
    """Decorator to register a class as an ECS component.

    IMPORTANT: The class MUST also inherit from Component. Both the decorator
    AND the inheritance are required - using only one will cause a runtime error.

    Args:
        storage: Explicit storage mode. Use ``"python"`` to opt into PyObject
            storage. When omitted, wrapper storage is required; if the
            component has non-primitive fields a ``TypeError`` is raised.

    Examples::

        @component
        class Velocity(Component):  # MUST inherit from Component
            x: float = 0.0
            y: float = 0.0

        @component(storage="python")
        @dataclass
        class Inventory(Component):
            items: list[str]
    """

    def _apply(cls: type[CT]) -> type[CT]:
        return _register_component(cls, storage=storage)

    # Called as @component (no parentheses) — cls is the class itself
    if cls is not None:
        return _register_component(cls, storage=storage)

    # Called as @component(...) — return the actual decorator
    return _apply


from .math import Vec2, Vec3

_PRIMITIVE_TYPES = (int, float, bool)
_WRAPPER_TYPES = (int, float, bool, Vec3, Vec2)


def _register_component(cls: type[CT], *, storage: str | None = None) -> type[CT]:
    """Internal implementation for the @component decorator."""
    from typing import get_type_hints

    if not issubclass(cls, Component):
        raise TypeError(
            f"{cls.__name__} must inherit from Component. Ensure that it has `class {cls.__name__}(Component):` syntax with the @component decorator."
        )

    # Check for data fields without @dataclass or custom __init__
    own_annotations = cls.__dict__.get("__annotations__", {})
    has_own_init = "__init__" in cls.__dict__
    if own_annotations and not dataclasses.is_dataclass(cls) and not has_own_init:
        raise TypeError(
            f"{cls.__name__} has data fields ({', '.join(own_annotations)}) but is not a dataclass. "
            f"Add @dataclass below @component, or remove the field annotations for a marker component.\n"
            f"  @component\n"
            f"  @dataclass\n"
            f"  class {cls.__name__}(Component):\n"
            f"      ..."
        )

    # Validate storage parameter
    if storage is not None and storage != "python":
        raise ValueError(
            f"Invalid storage mode '{storage}'. Use storage=\"python\" for PyObject storage, "
            f"or omit for automatic wrapper storage."
        )

    # Set explicit storage hint for the Rust side
    if storage == "python":
        cls.__pybevy_storage__ = "pyobject"  # type: ignore[attr-defined]
    else:
        # Check if non-primitive fields would force PyObject fallback.
        # Raise an error so the user explicitly opts in with storage="python".
        try:
            hints = get_type_hints(cls)
        except Exception:
            hints = getattr(cls, "__annotations__", {})

        non_primitive_fields = [
            (name, hint)
            for name, hint in hints.items()
            if not name.startswith("_") and hint not in _WRAPPER_TYPES
        ]

        if non_primitive_fields:
            field_list = ", ".join(
                f"'{name}' ({hint.__name__ if hasattr(hint, '__name__') else hint})"
                for name, hint in non_primitive_fields
            )
            raise TypeError(
                f"Component '{cls.__name__}' has non-primitive fields: {field_list}. "
                f"Non-primitive fields require PyObject storage, which disables View batch "
                f"execution and Numba JIT. Use @component(storage=\"python\") to opt in."
            )

    # Generate cache key from fully qualified name
    key = f"{cls.__module__}.{cls.__qualname__}"

    # Check if verbose mode is enabled
    verbose = os.environ.get("PYBEVY_VERBOSE") == "1"

    # Return cached component if caching is enabled and component exists in cache
    if _component_cache_enabled and key in _component_cache:
        if verbose:
            print(f"Using CACHED component: {key}")
        return _component_cache[key]  # type: ignore

    # Mark the component as properly decorated
    cls.__pybevy_component_decorated__ = True  # type: ignore[attr-defined]

    # Add from_numpy() classmethod for wrapper-storage components
    if storage != "python":

        @classmethod  # type: ignore[misc]  # dynamic classmethod on decorated class
        def from_numpy(klass: type, **kwargs: object) -> object:
            from .ecs import CustomComponentBatch

            return CustomComponentBatch(klass, **kwargs)

        cls.from_numpy = from_numpy

    # Auto-generate ViewColumn proxy class for batched View API
    _create_view_column_proxy(cls)

    # Store in cache for future lookups
    if _component_cache_enabled:
        if verbose:
            print(f"Caching NEW component: {key}")
        _component_cache[key] = cls
    else:
        if verbose:
            print(f"Using FRESH component (caching disabled): {key}")

    return cls


def _create_view_column_proxy(cls: type[Component]) -> None:
    """Auto-generate a ViewColumn proxy class for the component.

    This creates a class like:
        class MyComponentViewColumn:
            field1: FieldExpr
            field2: FieldExpr
            ...

    The proxy is stored as cls.__view_column_type__ for runtime access.
    """
    from typing import get_type_hints

    # Get field annotations
    try:
        hints = get_type_hints(cls)
    except Exception:
        # If type hints fail (e.g., forward references), fall back to __annotations__
        hints = getattr(cls, "__annotations__", {})

    # Create ViewColumn proxy class dynamically
    proxy_name = f"{cls.__name__}ViewColumn"
    proxy_attrs = {"__doc__": f"Auto-generated ViewColumn proxy for {cls.__name__}."}

    # Map each field to appropriate proxy type
    for field_name, field_type in hints.items():
        if field_name.startswith("_"):
            continue  # Skip private fields

        # Determine proxy type based on field type
        proxy_type = _get_proxy_type_for_field(field_type)
        proxy_attrs["__annotations__"] = proxy_attrs.get("__annotations__", {})
        proxy_attrs["__annotations__"][field_name] = proxy_type

    # Create the proxy class
    proxy_class = type(proxy_name, (), proxy_attrs)

    # Store on the component class for runtime access
    cls.__view_column_type__ = proxy_class

    # Also set it as a module-level attribute for imports (if possible)
    try:
        module = sys.modules.get(cls.__module__)
        if module:
            setattr(module, proxy_name, proxy_class)
    except Exception:
        pass  # Ignore if we can't set module attribute


def _get_proxy_type_for_field(field_type: type) -> str:
    """Determine the appropriate ViewColumn proxy type for a field.

    Returns:
        String name of the proxy type (for __annotations__)
    """

    # Handle simple primitive types
    if field_type in (int, float):
        return "FieldExpr"

    # Check for type name strings (common in annotations)
    type_name = getattr(field_type, "__name__", str(field_type))

    if "Vec3" in type_name:
        return "Vec3Expr"
    if "Vec2" in type_name:
        return "Vec2Expr"
    if "Quat" in type_name or "Quaternion" in type_name:
        return "QuatExpr"
    # Default to FieldExpr for unknown types
    return "FieldExpr"


def is_component_decorated(cls: type) -> bool:
    """Check if a class has been properly decorated with @component.

    Args:
        cls: The class to check

    Returns:
        True if the class has the @component decorator, False otherwise
    """
    return getattr(cls, "__pybevy_component_decorated__", False)


PL = TypeVar("PL", bound=Plugin)


def plugin(cls: type[PL]) -> type[PL]:
    """Decorator to register a class as a Bevy plugin.

    IMPORTANT: The class MUST also inherit from Plugin. Both the decorator
    AND the inheritance are required.

    Example:
        @plugin
        class MyGamePlugin(Plugin):  # MUST inherit from Plugin
            def build(self, app: App) -> None:
                app.add_systems(Startup, setup)
    """
    if not issubclass(cls, Plugin):
        raise TypeError(f"{cls.__name__} must inherit from Plugin.")

    # Mark the plugin as properly decorated
    # This allows us to detect plugins missing the @plugin decorator
    cls.__pybevy_plugin_decorated__ = True

    return cls


def material(
    fragment_shader: str | None = None,
    vertex_shader: str | None = None,
) -> Callable[[type], type]:
    """Decorator to define a custom shader material.

    See pybevy.material for full implementation and docs.
    """
    from .material import material as _material_impl
    return _material_impl(fragment_shader=fragment_shader, vertex_shader=vertex_shader)


def post_process(
    shader: str,
) -> Callable[[type], type]:
    """Decorator to define a post-processing effect.

    See pybevy.post_process for full implementation and docs.
    """
    from .post_process import post_process as _post_process_impl
    return _post_process_impl(shader=shader)


def entrypoint(func: Callable) -> Callable:
    """Decorator for app entry point functions.

    Automatically injects an App instance when called with no arguments,
    while still allowing explicit App instances for testing.

    Example:
        @entrypoint
        def main(app: App) -> App:
            return (
                app.add_plugins(DefaultPlugins)
                .add_systems(Startup, setup)
            )

        if __name__ == "__main__":
            main().run()  # App() is automatically created and injected

    For testing:
        test_app = App()
        test_app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
        main(test_app).run()  # Explicit app instance
    """
    from .app import App as AppClass

    @wraps(func)
    def wrapper(app: AppClass | None = None) -> AppClass:
        if app is None:
            app = AppClass()

        app._mark_entrypoint()

        # Redirect stdout to stderr when in pipe mode to protect JSON-RPC pipe
        if os.environ.get("PYBEVY_MCP_PIPE") == "1":
            sys.stdout = sys.stderr

        try:
            result = func(app)
        except Exception as e:
            # Add a clearer error message before re-raising
            # The traceback will be printed by the caller (CLI or test harness)
            print(f"\n❌ Error configuring app in '{func.__name__}()':", file=sys.stderr)
            print(f"   {type(e).__name__}: {e}", file=sys.stderr)
            print(file=sys.stderr)

            # Re-raise the exception so it can be handled by the caller
            raise

        # Auto-inject McpPlugin when launched via `pybevy mcp`
        if os.environ.get("PYBEVY_MCP") == "1":
            from .mcp import McpPlugin

            port = int(os.environ.get("PYBEVY_CONTROL_PORT", "8420"))
            result = result.add_plugins(McpPlugin(port=port, execute_python=True))

            # Auto-inject ImageCopyPlugin for headless screenshot support
            from ._internal.render_readback import (
                ImageCopyPlugin,  # type: ignore[attr-defined]
            )

            result = result.add_plugins(ImageCopyPlugin())

        return result

    return wrapper
