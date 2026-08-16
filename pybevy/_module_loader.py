import importlib.util
import os
import sys
from types import ModuleType


def load_sibling_module(
    native_module: ModuleType,
    qualified_name: str,
    path: str,
    *,
    optional: bool,
) -> ModuleType | None:
    """Load a Python sibling and never retain a half-initialized module."""
    spec = importlib.util.spec_from_file_location(qualified_name, path)
    if spec is None or spec.loader is None:
        return None

    module = importlib.util.module_from_spec(spec)
    sys.modules[qualified_name] = module
    try:
        spec.loader.exec_module(module)
    except Exception:
        if sys.modules.get(qualified_name) is module:
            del sys.modules[qualified_name]
        if optional:
            return None
        raise

    setattr(native_module, qualified_name.rsplit(".", 1)[-1], module)
    return module


def load_required_sibling_modules(
    native_module: ModuleType,
    package_name: str,
    directory: str,
) -> None:
    """Load every public Python sibling beside a native package module."""
    for filename in os.listdir(directory):
        if filename.startswith("_") or not filename.endswith(".py"):
            continue
        module_name = filename[:-3]
        qualified_name = f"{package_name}.{module_name}"
        if qualified_name not in sys.modules:
            load_sibling_module(
                native_module,
                qualified_name,
                os.path.join(directory, filename),
                optional=False,
            )
