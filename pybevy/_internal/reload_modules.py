"""Temporary import registrations owned by one native reload attempt."""

from __future__ import annotations

import os
import sys
import sysconfig
import threading
import weakref
from collections.abc import Callable, Mapping
from contextvars import ContextVar
from types import ModuleType
from typing import Protocol, cast

_ResourceAssignmentAliases = Mapping[str, tuple[weakref.ReferenceType[type], ...]]
_HintFreezer = Callable[[], None]
_resource_hint_freezers: tuple[tuple[str, weakref.WeakMethod[_HintFreezer]], ...] = ()
_resource_hint_freezers_lock = threading.Lock()


class _ResourceValidation(Protocol):
    def publish(self, aliases: _ResourceAssignmentAliases) -> None: ...


_active: ContextVar[ModuleReloadTransaction | None] = ContextVar(
    "pybevy_module_reload", default=None
)
_MISSING = object()


def _register_resource_assignment_hint_freezer(
    module_name: str,
    freeze: _HintFreezer,
) -> None:
    global _resource_hint_freezers
    reference = weakref.WeakMethod(freeze)
    with _resource_hint_freezers_lock:
        live = tuple(item for item in _resource_hint_freezers if item[1]() is not None)
        _resource_hint_freezers = (*live, (module_name, reference))


def _freeze_resource_assignment_hints(module_names: list[str]) -> None:
    global _resource_hint_freezers
    names = frozenset(module_names)
    with _resource_hint_freezers_lock:
        snapshot = _resource_hint_freezers
    for module_name, reference in snapshot:
        freeze = reference()
        if freeze is not None and module_name in names:
            freeze()
    with _resource_hint_freezers_lock:
        _resource_hint_freezers = tuple(
            item for item in _resource_hint_freezers if item[1]() is not None
        )


def _under_root(module: ModuleType, root: str) -> bool:
    path = getattr(module, "__file__", None)
    try:
        return isinstance(path, str | os.PathLike) and os.path.realpath(
            path
        ).startswith(root)
    except TypeError:
        return False


def _protected(name: str, module: ModuleType) -> bool:
    if name in ("pybevy", "_pybevy", "__main__") or name.startswith(
        ("pybevy.", "_pybevy.")
    ):
        return True
    path = getattr(module, "__file__", None)
    return isinstance(path, str) and (
        "site-packages" in path
        or "dist-packages" in path
        or os.path.realpath(path).startswith(
            os.path.realpath(sysconfig.get_paths()["stdlib"]) + os.sep
        )
    )


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


def record_module_flush(project_dir: str, names: list[str]) -> None:
    transaction = _active.get()
    if transaction is not None:
        _freeze_resource_assignment_hints(names)
        transaction.record_flush(project_dir, names)


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
