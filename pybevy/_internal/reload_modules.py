"""Temporary import registrations owned by one native reload attempt."""

from __future__ import annotations

import os
import site
import sys
import sysconfig
import threading
import weakref
from collections.abc import Callable, Mapping
from contextvars import ContextVar
from functools import lru_cache
from types import FunctionType, ModuleType
from typing import TYPE_CHECKING, Protocol, cast

if TYPE_CHECKING:
    from ..util.import_graph import ImportGraph

_ResourceAssignmentAliases = Mapping[str, tuple[weakref.ReferenceType[type], ...]]
_HintFreezer = Callable[[], None]
_annotation_hint_freezers: tuple[tuple[str, weakref.WeakMethod[_HintFreezer]], ...] = ()
_annotation_hint_freezers_lock = threading.Lock()


class _ResourceValidation(Protocol):
    def publish(self, aliases: _ResourceAssignmentAliases) -> None: ...


_active: ContextVar[ModuleReloadTransaction | None] = ContextVar(
    "pybevy_module_reload", default=None
)
_MISSING = object()
# Members the interpreter sets on every class object; a reload never copies them.
_CLASS_IDENTITY_MEMBERS = frozenset(
    {
        "__dict__",
        "__weakref__",
        "__module__",
        "__qualname__",
        "__annotations__",
        "__annotate__",
        "__annotate_func__",
        "__annotations_cache__",
        "__static_attributes__",
        "__firstlineno__",
        "__orig_bases__",
        "__parameters__",
        "__type_params__",
    }
)
# Members pybevy decorators install; any other one needs a `__pybevy_` name.
_PYBEVY_CLASS_MEMBERS = frozenset(
    {"batch", "__view_column_type__", "__new__", "__setattr__"}
)


def _register_annotation_hint_freezer(
    module_name: str,
    freeze: _HintFreezer,
) -> None:
    global _annotation_hint_freezers
    reference = weakref.WeakMethod(freeze)
    with _annotation_hint_freezers_lock:
        live = tuple(
            item for item in _annotation_hint_freezers if item[1]() is not None
        )
        _annotation_hint_freezers = (*live, (module_name, reference))


def _freeze_annotation_hints(module_names: list[str]) -> None:
    global _annotation_hint_freezers
    names = frozenset(module_names)
    with _annotation_hint_freezers_lock:
        snapshot = _annotation_hint_freezers
    for module_name, reference in snapshot:
        freeze = reference()
        if freeze is not None and module_name in names:
            freeze()
    with _annotation_hint_freezers_lock:
        _annotation_hint_freezers = tuple(
            item for item in _annotation_hint_freezers if item[1]() is not None
        )


def _under_root(module: ModuleType, root: str) -> bool:
    path = getattr(module, "__file__", None)
    try:
        return isinstance(path, str | os.PathLike) and os.path.realpath(
            path
        ).startswith(root)
    except TypeError:
        return False


@lru_cache(maxsize=1)
def _external_module_roots() -> tuple[str, ...]:
    paths = sysconfig.get_paths()
    roots = {paths[key] for key in ("stdlib", "purelib", "platlib") if paths.get(key)}
    get_site_packages = getattr(site, "getsitepackages", None)
    if get_site_packages is not None:
        roots.update(get_site_packages())
    user_site = site.getusersitepackages()
    if user_site:
        roots.add(user_site)
    return tuple(os.path.realpath(root) + os.sep for root in roots)


def is_external_module_path(path: str | os.PathLike[str]) -> bool:
    """Whether a module is under an actual stdlib or installed package root."""
    resolved = os.path.realpath(path)
    return any(resolved.startswith(root) for root in _external_module_roots())


def _protected(name: str, module: ModuleType) -> bool:
    if name in ("pybevy", "_pybevy", "__main__") or name.startswith(
        ("pybevy.", "_pybevy.")
    ):
        return True
    path = getattr(module, "__file__", None)
    try:
        return isinstance(path, str | os.PathLike) and is_external_module_path(path)
    except (TypeError, ValueError):
        return False


class ModuleReloadTransaction:
    def __init__(self) -> None:
        self._before = set(sys.modules)
        self._removed: dict[str, ModuleType] = {}
        self._roots: set[str] = set()
        self._bindings: dict[str, tuple[ModuleType, object]] = {}
        self._resource_alias_preparers: list[
            tuple[
                object,
                Callable[
                    [_ResourceAssignmentAliases | None], _ResourceAssignmentAliases
                ],
            ]
        ] = []
        self._resource_aliases: dict[object, _ResourceAssignmentAliases] = {}
        self._component_annotations: dict[type, dict[str, object]] = {}
        self._class_bodies: dict[type, dict[str, object]] = {}
        self._import_graph_updates: dict[int, tuple[ImportGraph, ImportGraph]] = {}
        self._token = _active.set(self)

    def stage_resource_assignment_alias(
        self,
        validation: object,
        prepare: Callable[
            [_ResourceAssignmentAliases | None], _ResourceAssignmentAliases
        ],
    ) -> None:
        self._resource_alias_preparers.append((validation, prepare))

    def prepare_resource_assignment_aliases(self) -> None:
        for validation, prepare in self._resource_alias_preparers:
            self._resource_aliases[validation] = prepare(
                self._resource_aliases.get(validation)
            )

    def stage_component_annotations(
        self, component_type: type, annotations: Mapping[str, object]
    ) -> None:
        self._component_annotations[component_type] = dict(annotations)

    def stage_class_body(self, target: type, body: Mapping[str, object]) -> None:
        self._class_bodies[target] = dict(body)

    def record_flush(self, project_dir: str, names: list[str]) -> None:
        root = os.path.realpath(project_dir) + os.sep
        self._roots.add(root)
        for name in names:
            if name not in self._before:
                continue
            previous = sys.modules.get(name)
            if isinstance(previous, ModuleType):
                self._removed.setdefault(name, previous)
            parent_name, _, child = name.rpartition(".")
            parent = self._removed.get(parent_name, sys.modules.get(parent_name))
            if isinstance(parent, ModuleType):
                self._bindings.setdefault(
                    name, (parent, vars(parent).get(child, _MISSING))
                )

    def finish(self, success: bool) -> None:
        try:
            if success:
                for validation, aliases in self._resource_aliases.items():
                    cast(_ResourceValidation, validation).publish(aliases)
                for component_type, annotations in self._component_annotations.items():
                    type.__setattr__(component_type, "__annotations__", annotations)
                for target, body in self._class_bodies.items():
                    _adopt_class_body(target, body)
                for graph, candidate in self._import_graph_updates.values():
                    graph.publish(candidate)
            else:
                for name, module in list(sys.modules.items()):
                    if (
                        name not in self._before
                        and isinstance(module, ModuleType)
                        and not _protected(name, module)
                        and any(_under_root(module, root) for root in self._roots)
                    ):
                        del sys.modules[name]
                        parent_name, _, child = name.rpartition(".")
                        parent = sys.modules.get(parent_name)
                        if (
                            isinstance(parent, ModuleType)
                            and vars(parent).get(child) is module
                        ):
                            del vars(parent)[child]
                sys.modules.update(self._removed)
                for name, (parent, value) in self._bindings.items():
                    child = name.rpartition(".")[2]
                    if value is _MISSING:
                        vars(parent).pop(child, None)
                    else:
                        vars(parent)[child] = value
        finally:
            _active.reset(self._token)
            self._before.clear()
            self._removed.clear()
            self._bindings.clear()
            self._roots.clear()
            self._resource_alias_preparers.clear()
            self._resource_aliases.clear()
            self._component_annotations.clear()
            self._class_bodies.clear()
            self._import_graph_updates.clear()

    def stage_import_graph(self, graph: ImportGraph, candidate: ImportGraph) -> None:
        """Publish a candidate dependency graph only if this reload commits."""
        self._import_graph_updates[id(graph)] = (graph, candidate)


def record_module_flush(project_dir: str, names: list[str]) -> None:
    transaction = _active.get()
    if transaction is not None:
        _freeze_annotation_hints(names)
        transaction.record_flush(project_dir, names)


def evict_modules(names: list[str]) -> None:
    """Remove modules from ``sys.modules`` and from their parent packages.

    ``from pkg import mod`` reads the parent's attribute before ``sys.modules``,
    so a binding left on a surviving parent keeps serving the evicted module.
    """
    for name in names:
        module = sys.modules.pop(name, None)
        parent_name, _, child = name.rpartition(".")
        parent = sys.modules.get(parent_name)
        if (
            module is not None
            and isinstance(parent, ModuleType)
            and vars(parent).get(child) is module
        ):
            del vars(parent)[child]


def stage_resource_assignment_alias(
    validation: object,
    prepare: Callable[[_ResourceAssignmentAliases | None], _ResourceAssignmentAliases],
) -> None:
    transaction = _active.get()
    if transaction is None:
        cast(_ResourceValidation, validation).publish(prepare(None))
        return
    transaction.stage_resource_assignment_alias(validation, prepare)
    transaction.prepare_resource_assignment_aliases()


def prepare_resource_assignment_aliases() -> None:
    transaction = _active.get()
    if transaction is not None:
        transaction.prepare_resource_assignment_aliases()


def resource_assignment_aliases(
    validation: object,
) -> _ResourceAssignmentAliases | None:
    transaction = _active.get()
    if transaction is None:
        return None
    return transaction._resource_aliases.get(validation)


def stage_component_annotations(
    component_type: type, annotations: Mapping[str, object]
) -> None:
    """Publish resolved cached-component annotations with the reload transaction."""
    transaction = _active.get()
    if transaction is None:
        type.__setattr__(component_type, "__annotations__", dict(annotations))
        return
    transaction.stage_component_annotations(component_type, annotations)


def _is_class_body_member(name: str) -> bool:
    return (
        name not in _CLASS_IDENTITY_MEMBERS
        and name not in _PYBEVY_CLASS_MEMBERS
        and not name.startswith("__pybevy_")
    )


def _functions_of(value: object) -> list[FunctionType]:
    if isinstance(value, classmethod | staticmethod):
        value = value.__func__
    if isinstance(value, property):
        return [
            f
            for f in (value.fget, value.fset, value.fdel)
            if isinstance(f, FunctionType)
        ]
    return [value] if isinstance(value, FunctionType) else []


def _adopt_class_body(target: type, body: Mapping[str, object]) -> None:
    for name, value in list(vars(target).items()):
        if name not in body and _is_class_body_member(name) and _functions_of(value):
            type.__delattr__(target, name)
    for name, value in body.items():
        for function in _functions_of(value):
            # Zero-argument super() reads this cell; point it at the kept class.
            if "__class__" in function.__code__.co_freevars and function.__closure__:
                cell = function.__closure__[
                    function.__code__.co_freevars.index("__class__")
                ]
                cell.cell_contents = target
        type.__setattr__(target, name, value)


def stage_class_body(target: type, source: type) -> None:
    """Give a cached class the reloaded class body once the reload commits.

    The cached class keeps its identity for live instances and Bevy
    registrations; its methods, properties and dataclass members follow the
    re-executed source.
    """
    body = {
        name: value
        for name, value in vars(source).items()
        if _is_class_body_member(name)
    }
    transaction = _active.get()
    if transaction is None:
        _adopt_class_body(target, body)
        return
    transaction.stage_class_body(target, body)


def stage_import_graph_update(graph: ImportGraph, candidate: ImportGraph) -> None:
    transaction = _active.get()
    if transaction is None:
        graph.publish(candidate)
    else:
        transaction.stage_import_graph(graph, candidate)
