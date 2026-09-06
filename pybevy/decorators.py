import dataclasses
import os
import sys
import types
from collections.abc import Callable
from functools import wraps
from typing import Any, TypeVar, Union, get_args, get_origin, get_type_hints

from .app import Plugin
from .ecs import Component, Event, Message, Resource
from .material import material as material

RT = TypeVar("RT", bound=Resource)
MT = TypeVar("MT", bound=Message)
ET = TypeVar("ET", bound=Event)

_component_cache: dict[str, type[Component]] = {}
_component_layout_signatures: dict[str, tuple[object, ...]] = {}
_component_layout_reload_required: set[str] = set()
_resource_cache: dict[str, type[Resource]] = {}
_component_cache_enabled = False


def message(cls: type[MT]) -> type[MT]:
    """Validate and mark a custom buffered-message class.

    The class must still inherit from :class:`Message`, and its channel must
    still be registered with ``app.add_message(cls)``. The decorator is a
    declaration aid; it does not mutate an App or register global state.
    """
    if not issubclass(cls, Message):
        raise TypeError(f"{cls.__name__} must inherit from Message.")
    cls.__pybevy_message_decorated__ = True
    return cls


def event(cls: type[ET]) -> type[ET]:
    """Validate and mark a custom observer-event class.

    The class must still inherit from :class:`Event`. Observers remain
    explicitly registered with ``app.add_observer(...)``.
    """
    if not issubclass(cls, Event):
        raise TypeError(f"{cls.__name__} must inherit from Event.")
    cls.__pybevy_event_decorated__ = True
    return cls


def _annotation_name(annotation: object) -> str:
    if annotation is type(None):
        return "None"
    origin = get_origin(annotation)
    if origin in (types.UnionType, Union):
        return " | ".join(_annotation_name(member) for member in get_args(annotation))
    if origin is not None:
        return str(annotation).removeprefix("typing.")
    return getattr(annotation, "__name__", str(annotation).removeprefix("typing."))


def _validated_resource_value(annotation: object, value: object) -> tuple[bool, object]:
    if annotation is Any:
        return True, value
    if annotation is int:
        return type(value) is int, value
    if annotation is float:
        if type(value) is int:
            return True, float(value)
        return type(value) is float, value
    if annotation is bool:
        return type(value) is bool, value
    if annotation is str:
        return type(value) is str, value
    if annotation is None or annotation is type(None):
        return value is None, value

    origin = get_origin(annotation)
    if origin in (types.UnionType, Union):
        for member in get_args(annotation):
            valid, converted = _validated_resource_value(member, value)
            if valid:
                return True, converted
        return False, value
    if origin is not None:
        try:
            return isinstance(value, origin), value
        except TypeError:
            return True, value
    if isinstance(annotation, type):
        return isinstance(value, annotation), value
    return True, value


def _install_resource_assignment_validation(cls: type[RT]) -> None:
    original_setattr = cls.__setattr__
    resolved_hints: dict[str, object] | None = None

    def checked_setattr(instance: RT, name: str, value: object) -> None:
        nonlocal resolved_hints
        if resolved_hints is None:
            try:
                resolved_hints = get_type_hints(type(instance))
            except (NameError, TypeError):
                resolved_hints = dict(getattr(type(instance), "__annotations__", {}))

        annotation = resolved_hints.get(name)
        if annotation is not None:
            valid, value = _validated_resource_value(annotation, value)
            if not valid:
                raise TypeError(
                    f"{name}: expected {_annotation_name(annotation)}, "
                    f"got {type(value).__name__}"
                )
        original_setattr(instance, name, value)

    cls.__setattr__ = checked_setattr  # type: ignore[method-assign,assignment]


def resource(cls: type[RT]) -> type[RT]:
    """Decorator to register a class as a global ECS resource.

    IMPORTANT: The class MUST also inherit from Resource. Both the decorator
    AND the inheritance are required - using only one will cause a runtime error.

    Example:
        from dataclasses import dataclass

        @resource
        @dataclass
        class GameState(Resource):  # MUST inherit from Resource
            score: int = 0
    """
    if not issubclass(cls, Resource):
        raise TypeError(f"{cls.__name__} must inherit from Resource.")

    key = f"{cls.__module__}.{cls.__qualname__}"
    if _component_cache_enabled and key in _resource_cache:
        return _resource_cache[key]  # type: ignore[return-value]

    if not dataclasses.is_dataclass(cls) and cls.__init__ is object.__init__:
        original_new = cls.__new__

        def guarded_new(resource_type: type[RT], *args: object, **kwargs: object) -> RT:
            if (
                (args or kwargs)
                and not dataclasses.is_dataclass(resource_type)
                and resource_type.__init__ is object.__init__
            ):
                fields = getattr(resource_type, "__annotations__", {})
                field_hint = f" for its data fields ({', '.join(fields)})" if fields else ""
                raise TypeError(
                    f"{resource_type.__name__} does not define a constructor{field_hint}. "
                    "Add @dataclass below @resource, or define __init__ before passing "
                    "constructor arguments."
                )
            return original_new(resource_type)

        cls.__new__ = staticmethod(guarded_new)  # type: ignore[method-assign]

    # Mark the resource as properly decorated
    # This allows us to detect resources missing the @resource decorator
    cls.__pybevy_resource_decorated__ = True
    _install_resource_assignment_validation(cls)

    if _component_cache_enabled:
        _resource_cache[key] = cls

    return cls


CT = TypeVar("CT", bound=Component)

def clear_component_cache(verbose: bool = False) -> None:
    """Clear cached custom ECS types before a full reload."""
    global _component_cache, _component_layout_signatures, _resource_cache
    if verbose:
        print(
            "🗑️  Clearing ECS type cache "
            f"({len(_component_cache)} components, {len(_resource_cache)} resources)"
        )
    _component_cache.clear()
    _component_layout_signatures.clear()
    _resource_cache.clear()
    # Avoid importing pybevy.material eagerly (it imports native rendering types).
    # If it has been used, a full reload must invalidate its logical class aliases too.
    material_module = sys.modules.get("pybevy.material")
    if material_module is not None:
        clear_material_cache = getattr(
            material_module, "_clear_material_type_cache", None
        )
        if clear_material_cache is not None:
            clear_material_cache()
    if verbose:
        print("✅ ECS type cache cleared")


def _component_layout_reload_names() -> tuple[str, ...]:
    return tuple(sorted(_component_layout_reload_required))


def _commit_component_layout_reload() -> None:
    _component_layout_reload_required.clear()


def enable_component_caching(enabled: bool, verbose: bool = False) -> None:
    """Enable or disable custom ECS type caching.

    When enabled (partial reload mode), component classes are cached by qualified name
    and resource classes retain the same identity across reloads.

    When disabled (full reload mode), components are freshly registered each time.
    """
    global _component_cache_enabled
    prev_state = _component_cache_enabled
    _component_cache_enabled = enabled
    material_module = sys.modules.get("pybevy.material")
    if material_module is not None:
        set_material_caching = getattr(
            material_module, "_set_material_type_caching", None
        )
        if set_material_caching is not None:
            set_material_caching(enabled)
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

    # Called as @component (no parentheses): cls is the class itself
    if cls is not None:
        return _register_component(cls, storage=storage)

    # Called as @component(...): return the actual decorator
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
    own_annotations = cls.__annotations__
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
    try:
        field_hints = get_type_hints(cls)
    except Exception:
        field_hints = getattr(cls, "__annotations__", {})
    layout_signature = (
        "python" if storage == "python" else "wrapper",
        tuple(
            (name, _component_annotation_identity(annotation))
            for name, annotation in field_hints.items()
            if not name.startswith("_")
        ),
    )

    # Check if verbose mode is enabled
    verbose = os.environ.get("PYBEVY_VERBOSE") == "1"

    # Return cached component if caching is enabled and component exists in cache
    if _component_cache_enabled and key in _component_cache:
        if _component_layout_signatures.get(key) == layout_signature:
            if verbose:
                print(f"Using CACHED component: {key}")
            return _component_cache[key]  # type: ignore
        _component_layout_reload_required.add(key)
        if verbose:
            print(f"Component layout changed; replacing cached component: {key}")

    # Mark the component as properly decorated
    cls.__pybevy_component_decorated__ = True  # type: ignore[attr-defined]

    # Add from_numpy() classmethod for wrapper-storage components
    if storage == "python":

        @classmethod  # type: ignore[misc]  # dynamic classmethod on decorated class
        def from_numpy(klass: type, **_kwargs: object) -> object:
            raise TypeError(
                f'{klass.__name__}.from_numpy() is not supported for storage="python" components; '
                "spawn component instances instead"
            )

        cls.from_numpy = from_numpy
    else:

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
        _component_layout_signatures[key] = layout_signature
    else:
        if verbose:
            print(f"Using FRESH component (caching disabled): {key}")

    return cls


def _component_annotation_identity(annotation: object) -> object:
    origin = get_origin(annotation)
    if origin is not None:
        return (
            _component_annotation_identity(origin),
            tuple(_component_annotation_identity(arg) for arg in get_args(annotation)),
        )
    module = getattr(annotation, "__module__", None)
    qualname = getattr(annotation, "__qualname__", None)
    if module is not None and qualname is not None:
        return module, qualname
    return repr(annotation)


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
        auto_created = app is None
        if auto_created:
            from .util.hot_reload import _is_executing_scene_module

            if _is_executing_scene_module():
                raise RuntimeError(
                    f"{func.__name__}() was called while PyBevy was loading the scene "
                    "module. Keep the auto-created app runner behind "
                    '`if __name__ == "__main__":`.'
                )
        if app is None:
            app = AppClass()

        app._mark_entrypoint()

        # Redirect stdout to stderr when in pipe mode to protect JSON-RPC pipe
        if os.environ.get("PYBEVY_MCP_PIPE") == "1":
            sys.stdout = sys.stderr

        try:
            result = func(app)
        except Exception as e:
            from . import _pybevy  # type: ignore

            _pybevy._enrich_exception(e)
            print(f"\n❌ Error configuring app in '{func.__name__}()':", file=sys.stderr)
            print(f"   {type(e).__name__}: {e}", file=sys.stderr)
            print(file=sys.stderr)

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
