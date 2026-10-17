"""Source loader used by the embedded native plugin's reload transaction."""

from __future__ import annotations

import importlib
import importlib.util
import os
import sys
from collections.abc import Callable, Sequence
from importlib.abc import InspectLoader
from typing import TYPE_CHECKING

from pybevy._internal.reload_modules import (
    evict_modules,
    is_external_module_path,
    record_module_flush,
)

if TYPE_CHECKING:
    from pybevy.app import App, Stage


def _flush_user_modules(project_dir: str) -> None:
    project_prefix = os.path.realpath(project_dir) + os.sep
    to_remove = []
    for name, module in list(sys.modules.items()):
        if name in ("pybevy", "_pybevy", "__main__") or name.startswith(
            ("pybevy.", "_pybevy.")
        ):
            continue
        path = getattr(module, "__file__", None)
        if path is None:
            continue
        path = os.path.realpath(path)
        if path.startswith(project_prefix) and not is_external_module_path(path):
            to_remove.append(name)
    record_module_flush(project_dir, to_remove)
    evict_modules(to_remove)
    importlib.invalidate_caches()


def make_native_loader(
    module_name: str,
    systems: Sequence[tuple[str, Stage, bool]],
    plugins: Sequence[str] = (),
) -> Callable[[], Callable[[App], App]]:
    def loader() -> Callable[[App], App]:
        spec = importlib.util.find_spec(module_name)
        if spec is None or spec.origin is None:
            raise ImportError(f"Cannot find source for module '{module_name}'")

        _flush_user_modules(os.path.dirname(os.path.realpath(spec.origin)))
        fresh_spec = importlib.util.spec_from_file_location(module_name, spec.origin)
        if fresh_spec is None or not isinstance(fresh_spec.loader, InspectLoader):
            raise ImportError(
                f"Cannot build spec for module '{module_name}' at {spec.origin}"
            )

        module = importlib.util.module_from_spec(fresh_spec)
        # Circular imports and dataclass introspection need registration during execution.
        sys.modules[module_name] = module
        try:
            # Timestamp-based bytecode can miss same-second, same-size edits.
            source = fresh_spec.loader.get_source(module_name)
            if source is None:
                raise ImportError(
                    f"Cannot read source for module '{module_name}' at {spec.origin}"
                )
            exec(compile(source, spec.origin, "exec", dont_inherit=True), module.__dict__)
        except BaseException:
            sys.modules.pop(module_name, None)
            raise

        parent_name, _, child_name = module_name.rpartition(".")
        if parent_name:
            parent = importlib.import_module(parent_name)
            setattr(parent, child_name, module)

        def configurator(app: App) -> App:
            for plugin_name in plugins:
                app.add_plugins(getattr(module, plugin_name))
            for function_name, stage, optional in systems:
                if optional and function_name not in module.__dict__:
                    continue
                app.add_systems(stage, getattr(module, function_name))
            return app

        return configurator

    return loader
