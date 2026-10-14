"""Temporary import registrations owned by one native reload attempt."""

from __future__ import annotations

import hashlib
import os
import site
import sys
import sysconfig
import threading
import weakref
from collections.abc import Callable, Mapping, MutableMapping
from contextvars import ContextVar
from functools import lru_cache
from types import CodeType, FunctionType, ModuleType
from typing import TYPE_CHECKING, Protocol, cast

if TYPE_CHECKING:
    from ..util.import_graph import ImportGraph

_ResourceAssignmentAliases = Mapping[str, tuple[weakref.ReferenceType[type], ...]]
_HintFreezer = Callable[[], None]
_annotation_hint_freezers: tuple[tuple[str, weakref.WeakMethod[_HintFreezer]], ...] = ()
_annotation_hint_freezers_lock = threading.Lock()
# Defining module of every registered component class, by `module.qualname`.
_component_modules: dict[str, str] = {}
_component_forgetters: list[Callable[[list[str]], None]] = []


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
        self._entrypoint_fingerprint: str | None = None
        self._flushed: set[str] = set()
        self._components_defined: set[str] = set()
        self._components_removed: list[str] = []
        self._registry_undo: dict[tuple[int, str], Callable[[], None]] = {}
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
        self._flushed.update(names)
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

    def note_component_defined(self, key: str) -> None:
        self._components_defined.add(key)

    def removed_component_names(self) -> list[str]:
        self._components_removed = sorted(
            key
            for key, module in _component_modules.items()
            if module in self._flushed and key not in self._components_defined
        )
        return self._components_removed

    def finish(self, success: bool) -> None:
        try:
            if success:
                for key in self._components_removed:
                    _component_modules.pop(key, None)
                for forget in _component_forgetters:
                    forget(self._components_removed)
                for validation, aliases in self._resource_aliases.items():
                    cast(_ResourceValidation, validation).publish(aliases)
                for component_type, annotations in self._component_annotations.items():
                    type.__setattr__(component_type, "__annotations__", annotations)
                for target, body in self._class_bodies.items():
                    _adopt_class_body(target, body)
                for graph, candidate in self._import_graph_updates.values():
                    graph.publish(candidate)
                if self._entrypoint_fingerprint is not None:
                    _committed_entrypoint[0] = self._entrypoint_fingerprint
            else:
                for restore in reversed(self._registry_undo.values()):
                    restore()
                for graph, candidate in self._import_graph_updates.values():
                    graph.remember_failed_candidate(candidate)
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
            self._flushed.clear()
            self._components_defined.clear()
            self._components_removed = []
            self._registry_undo.clear()

    def stage_import_graph(self, graph: ImportGraph, candidate: ImportGraph) -> None:
        """Publish a candidate dependency graph only if this reload commits."""
        self._import_graph_updates[id(graph)] = (graph, candidate)


def record_module_flush(project_dir: str, names: list[str]) -> None:
    transaction = _active.get()
    if transaction is not None:
        _freeze_annotation_hints(names)
        transaction.record_flush(project_dir, names)


def register_component_forgetter(forget: Callable[[list[str]], None]) -> None:
    """Called with the component keys a committed reload found removed."""
    if forget not in _component_forgetters:
        _component_forgetters.append(forget)


def note_component_defined(key: str, module: str) -> None:
    record_mapping_entry(_component_modules, key)
    _component_modules[key] = module
    transaction = _active.get()
    if transaction is not None:
        transaction.note_component_defined(key)


def record_mapping_entry[T](mapping: MutableMapping[str, T], key: str) -> None:
    """Retain the first value a reload attempt displaces until it commits."""
    transaction = _active.get()
    if transaction is None or (id(mapping), key) in transaction._registry_undo:
        return
    if key in mapping:
        previous = mapping[key]

        def restore() -> None:
            mapping[key] = previous

    else:

        def restore() -> None:
            mapping.pop(key, None)

    transaction._registry_undo[id(mapping), key] = restore


def record_mapping_entries[T](mapping: MutableMapping[str, T]) -> None:
    """Record the entries a candidate's cache clear will remove."""
    for key in mapping:
        record_mapping_entry(mapping, key)


def record_set_member(values: set[str], key: str) -> None:
    """Restore a reload flag's previous membership if its candidate fails."""
    transaction = _active.get()
    if transaction is None or (id(values), key) in transaction._registry_undo:
        return
    present = key in values

    def restore() -> None:
        if present:
            values.add(key)
        else:
            values.discard(key)

    transaction._registry_undo[id(values), key] = restore


def removed_component_names() -> list[str]:
    """Component classes that a module re-executed by this reload no longer defines."""
    transaction = _active.get()
    return [] if transaction is None else transaction.removed_component_names()


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


_committed_entrypoint: list[str | None] = [None]


def _code_shape(code: CodeType) -> tuple[object, ...]:
    return (
        code.co_code,
        code.co_names,
        code.co_varnames,
        tuple(
            _code_shape(const) if isinstance(const, CodeType) else repr(const)
            for const in code.co_consts
        ),
    )


def code_fingerprint(code: CodeType) -> str:
    """Hash a function's code, ignoring line positions so comment edits do not count."""
    return hashlib.sha256(repr(_code_shape(code)).encode()).hexdigest()


def stage_entrypoint_fingerprint(fingerprint: str) -> None:
    transaction = _active.get()
    if transaction is None:
        _committed_entrypoint[0] = fingerprint
    else:
        transaction._entrypoint_fingerprint = fingerprint


def entrypoint_changed() -> bool:
    """Whether the reloading entrypoint differs from the last committed one."""
    transaction = _active.get()
    if transaction is None:
        return False
    candidate = transaction._entrypoint_fingerprint
    committed = _committed_entrypoint[0]
    return candidate is not None and committed is not None and candidate != committed
