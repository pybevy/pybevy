"""Temporary import registrations owned by one native reload attempt."""

import os
import sys
import sysconfig
from contextvars import ContextVar
from types import ModuleType

_active: ContextVar["ModuleReloadTransaction | None"] = ContextVar(
    "pybevy_module_reload", default=None
)
_MISSING = object()


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
        self._token = _active.set(self)

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
            if not success:
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


def record_module_flush(project_dir: str, names: list[str]) -> None:
    transaction = _active.get()
    if transaction is not None:
        transaction.record_flush(project_dir, names)
