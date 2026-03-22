import importlib.util
import os
import sys

import pybevy._pybevy as _pybevy  # type: ignore

_native = _pybevy.mcp
_mcp_dir = os.path.dirname(__file__)
_native.__path__ = [_mcp_dir]  # type: ignore
_native.__package__ = __name__  # type: ignore
sys.modules[__name__] = _native  # type: ignore

# Load Python submodules (decorators, schema) so they're accessible
# alongside the native Rust module.
_package_name = "pybevy.mcp"
for _fname in os.listdir(_mcp_dir):
    if _fname.startswith("_") or not _fname.endswith(".py"):
        continue
    _mod_name = _fname[:-3]
    _qual = f"{_package_name}.{_mod_name}"
    if _qual not in sys.modules:
        try:
            _spec = importlib.util.spec_from_file_location(
                _qual, os.path.join(_mcp_dir, _fname)
            )
            if _spec and _spec.loader:
                _mod = importlib.util.module_from_spec(_spec)
                sys.modules[_qual] = _mod
                _spec.loader.exec_module(_mod)
                setattr(_native, _mod_name, _mod)
        except Exception:
            pass

