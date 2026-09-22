import dataclasses
import inspect
import os
import sys
import threading
import types
import weakref
from collections.abc import Callable, Mapping
from functools import wraps
from typing import Any, TypeVar, Union, get_args, get_origin, get_type_hints
from weakref import WeakValueDictionary

from ._internal.reload_modules import _register_resource_assignment_hint_freezer
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
_resource_layout_signatures: dict[str, tuple[object, ...]] = {}
_resource_layout_reload_required: set[str] = set()
_component_cache_enabled = False

# Decorated component classes by `module.qualname`. Weak so a reloaded scene
# module's classes stay collectable.
_component_classes_by_name: WeakValueDictionary[str, type[Component]] = (
    WeakValueDictionary()
)


def _component_class_by_name(qualified_name: str) -> type[Component] | None:
    """Resolve a decorated component class by its ``module.qualname``.

    World deserialization meets a custom component by name before anything
    spawns it, so the class has to be findable from decoration time onwards.
    """
    return _component_classes_by_name.get(qualified_name)


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


_ResourceAssignmentAliases = Mapping[str, tuple[weakref.ReferenceType[type], ...]]
_RESOURCE_ASSIGNMENT_VALIDATION_ATTR = "__pybevy_resource_assignment_validation__"


def _runtime_validation_types(annotation: object) -> tuple[type, ...]:
    origin = get_origin(annotation)
    if origin in (types.UnionType, Union):
        return tuple(
            candidate
            for member in get_args(annotation)
            for candidate in _runtime_validation_types(member)
        )
    if isinstance(origin, type):
        return (origin,)
    if isinstance(annotation, type):
        return (annotation,)
    return ()


def _resolved_resource_hints(cls: type) -> dict[str, object] | None:
    try:
        return get_type_hints(cls)
    except (NameError, TypeError):
        return None


class _ResourceAssignmentValidation[RT]:
    def __init__(
        self,
        cls: type[RT],
        original_setattr: Callable[[RT, str, object], None],
    ) -> None:
        self._cls = weakref.ref(cls)
        self._original_setattr = original_setattr
        self._raw_hints = dict(getattr(cls, "__annotations__", {}))
        self._resolved_hints = _resolved_resource_hints(cls)
        self._hints_frozen = self._resolved_hints is not None
        self._hints_lock = threading.Lock()
        self._committed: _ResourceAssignmentAliases = types.MappingProxyType({})
        self._publish_lock = threading.Lock()

    def freeze_hints(self) -> None:
        with self._hints_lock:
            if self._resolved_hints is None and not self._hints_frozen:
                cls = self._cls()
                if cls is not None:
                    self._resolved_hints = _resolved_resource_hints(cls)
            self._hints_frozen = True

    def _hints(self) -> dict[str, object]:
        with self._hints_lock:
            if self._resolved_hints is None and not self._hints_frozen:
                cls = self._cls()
                if cls is not None:
                    self._resolved_hints = _resolved_resource_hints(cls)
        return self._resolved_hints or self._raw_hints

    def prepare(
        self,
        fresh_cls: type[RT],
        previous: _ResourceAssignmentAliases | None = None,
    ) -> _ResourceAssignmentAliases:
        fresh_hints = _resolved_resource_hints(fresh_cls) or dict(
            getattr(fresh_cls, "__annotations__", {})
        )
        source = self._committed if previous is None else previous
        prepared: dict[str, tuple[weakref.ReferenceType[type], ...]] = {
            name: tuple(
                reference for reference in references if reference() is not None
            )
            for name, references in source.items()
        }
        with self._hints_lock:
            baseline = self._resolved_hints or self._raw_hints
        for name, annotation in fresh_hints.items():
            references = list(prepared.get(name, ()))
            known = {
                candidate
                for reference in references
                if (candidate := reference()) is not None
            }
            known.update(_runtime_validation_types(baseline.get(name)))
            for candidate in _runtime_validation_types(annotation):
                if candidate not in known:
                    references.append(weakref.ref(candidate))
                    known.add(candidate)
            prepared[name] = tuple(references)
        return types.MappingProxyType(prepared)

    def publish(self, aliases: _ResourceAssignmentAliases) -> None:
        with self._publish_lock:
            self._committed = aliases

    def set_value(self, instance: RT, name: str, value: object) -> None:
        cls = self._cls()
        instance_type = type(instance)
        instance_validation = vars(instance_type).get(
            _RESOURCE_ASSIGNMENT_VALIDATION_ATTR
        )
        if cls is not instance_type and instance_validation is not self:
            if instance_validation is not None:
                self._original_setattr(instance, name, value)
                return
            hints = _resolved_resource_hints(instance_type) or dict(
                getattr(instance_type, "__annotations__", {})
            )
        else:
            hints = self._hints()

        annotation = hints.get(name)
        if annotation is not None:
            valid, value = _validated_resource_value(annotation, value)
            if not valid:
                from ._internal.reload_modules import resource_assignment_aliases

                aliases = resource_assignment_aliases(self) or self._committed
                valid = any(
                    isinstance(value, candidate)
                    for reference in aliases.get(name, ())
                    if (candidate := reference()) is not None
                )
            if not valid:
                raise TypeError(
                    f"{name}: expected {_annotation_name(annotation)}, "
                    f"got {type(value).__name__}"
                )
        self._original_setattr(instance, name, value)


def _install_resource_assignment_validation(cls: type[RT]) -> None:
    validation = _ResourceAssignmentValidation(cls, cls.__setattr__)
    _register_resource_assignment_hint_freezer(cls.__module__, validation.freeze_hints)

    def checked_setattr(instance: RT, name: str, value: object) -> None:
        validation.set_value(instance, name, value)

    cls.__pybevy_resource_assignment_validation__ = validation  # type: ignore[attr-defined]
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
    layout_signature = _resource_layout_signature(cls)

    # Full reload drops classes, but the next Partial still needs their layouts.
    previous_signature = _resource_layout_signatures.get(key)
    if previous_signature is not None and previous_signature != layout_signature:
        _resource_layout_reload_required.add(key)
    elif _component_cache_enabled and key in _resource_cache:
        cached = _resource_cache[key]
        from ._internal.reload_modules import stage_resource_assignment_alias

        validation: _ResourceAssignmentValidation[RT] = (
            cached.__pybevy_resource_assignment_validation__  # type: ignore[attr-defined]
        )
        stage_resource_assignment_alias(
            validation,
            lambda previous: validation.prepare(cls, previous),
        )
        return cached  # type: ignore[return-value]

    if not dataclasses.is_dataclass(cls) and cls.__init__ is object.__init__:
        original_new = cls.__new__

        def guarded_new(resource_type: type[RT], *args: object, **kwargs: object) -> RT:
            if (
                (args or kwargs)
                and not dataclasses.is_dataclass(resource_type)
                and resource_type.__init__ is object.__init__
            ):
                fields = getattr(resource_type, "__annotations__", {})
                field_hint = (
                    f" for its data fields ({', '.join(fields)})" if fields else ""
                )
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

    _resource_layout_signatures[key] = layout_signature
    if _component_cache_enabled:
        _resource_cache[key] = cls

    return cls


def _resource_layout_signature(cls: type) -> tuple[object, ...]:
    try:
        field_hints = get_type_hints(cls)
    except (NameError, TypeError):
        field_hints = getattr(cls, "__annotations__", {})
    return tuple(
        (name, _component_annotation_identity(annotation))
        for name, annotation in field_hints.items()
    )


CT = TypeVar("CT", bound=Component)


def clear_component_cache(verbose: bool = False) -> None:
    """Clear cached ECS classes, retaining layout signatures for reload comparison."""
    global _component_cache, _resource_cache
    if verbose:
        print(
            "🗑️  Clearing ECS type cache "
            f"({len(_component_cache)} components, {len(_resource_cache)} resources)"
        )
    _component_cache.clear()
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


def _resource_layout_reload_names() -> tuple[str, ...]:
    return tuple(sorted(_resource_layout_reload_required))


def _commit_resource_layout_reload() -> None:
    _resource_layout_reload_required.clear()


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
                f'execution and Numba JIT. Use @component(storage="python") to opt in.'
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

    # Layout comparison is independent of the class cache: a full reload drops
    # the cached classes, and the partial reload after it must still see a
    # changed layout.
    previous_signature = _component_layout_signatures.get(key)
    if previous_signature is not None and previous_signature != layout_signature:
        _component_layout_reload_required.add(key)
        if verbose:
            print(f"Component layout changed; replacing cached component: {key}")
    elif _component_cache_enabled and key in _component_cache:
        if verbose:
            print(f"Using CACHED component: {key}")
        return _component_cache[key]  # type: ignore

    # Mark the component as properly decorated
    cls.__pybevy_component_decorated__ = True  # type: ignore[attr-defined]

    # Make the class resolvable by name before anything spawns it.
    _component_classes_by_name[key] = cls

    # Add batch() classmethod for wrapper-storage components
    if storage == "python":

        @classmethod  # type: ignore[misc]  # dynamic classmethod on decorated class
        def batch(klass: type, **_kwargs: object) -> object:
            raise TypeError(
                f'{klass.__name__}.batch() is not supported for storage="python" components; '
                "spawn component instances instead"
            )

        cls.batch = batch
    else:

        @classmethod  # type: ignore[misc]  # dynamic classmethod on decorated class
        def batch(klass: type, **kwargs: object) -> object:
            from .ecs import CustomComponentBatch

            return CustomComponentBatch(klass, **kwargs)

        cls.batch = batch

    # Auto-generate ViewColumn proxy class for batched View API
    _create_view_column_proxy(cls)

    # Recorded even with caching disabled, so a full reload still leaves the
    # next partial reload something to compare against.
    _component_layout_signatures[key] = layout_signature

    # Store in cache for future lookups
    if _component_cache_enabled:
        if verbose:
            print(f"Caching NEW component: {key}")
        _component_cache[key] = cls
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


def _require_entrypoint_signature(func: Callable) -> None:
    """The entrypoint callable must accept one positional App."""
    name = getattr(func, "__name__", None)
    if name is None:
        name = type(func).__name__
    if inspect.iscoroutinefunction(func):
        raise TypeError(
            f"@entrypoint '{name}' must be synchronous and return an App; "
            f"use 'def {name}(app: App) -> App', not 'async def'."
        )
    try:
        signature = inspect.signature(func)
    except (TypeError, ValueError) as error:
        raise TypeError(
            f"@entrypoint cannot inspect callable '{name}'; use a Python function accepting an App."
        ) from error
    try:
        signature.bind(object())
    except TypeError as error:
        raise TypeError(
            f"@entrypoint requires a callable accepting one positional App: "
            f"'def {name}(app: App) -> App'. Got 'def {name}{signature}'."
        ) from error


def entrypoint(func: Callable) -> Callable:
    """Decorator for app entry point functions.

    Automatically injects an App instance when called with no arguments,
    while still allowing explicit App instances for testing. The callable must
    have an inspectable signature that accepts one positional App; optional
    parameters and positional variadics are supported.

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
        test_app.add_plugins(ScheduleRunnerPlugin.run_once())
        main(test_app).run()  # Explicit app instance
    """
    from .app import App as AppClass

    _require_entrypoint_signature(func)

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
            if not isinstance(result, AppClass):
                if inspect.iscoroutine(result):
                    result.close()
                raise TypeError(
                    f"@entrypoint '{func.__name__}' must return an App, "
                    f"got {type(result).__name__}; return the configured app."
                )
        except Exception as e:
            from . import _pybevy  # type: ignore

            _pybevy._enrich_exception(e)
            print(
                f"\n❌ Error configuring app in '{func.__name__}()':", file=sys.stderr
            )
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

    wrapper.__pybevy_entrypoint_decorated__ = weakref.ref(wrapper)  # type: ignore[attr-defined]

    return wrapper
