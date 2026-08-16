import os
import sys

from .. import _pybevy  # type: ignore
from .._module_loader import load_required_sibling_modules as _load_required_siblings

_native = _pybevy.mcp
_mcp_dir = os.path.dirname(__file__)
_native.__name__ = __name__  # type: ignore
_native.__path__ = [_mcp_dir]  # type: ignore
_native.__package__ = __name__  # type: ignore
sys.modules[__name__] = _native  # type: ignore

# Load Python submodules (decorators, schema) so they're accessible alongside
# the native Rust module.
_load_required_siblings(_native, "pybevy.mcp", _mcp_dir)
