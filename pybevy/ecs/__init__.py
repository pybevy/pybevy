import os
import sys

from .. import _pybevy  # type: ignore
from .._module_loader import load_sibling_module as _load_sibling_module
from ._system_sets import install_system_set_api as _install_system_set_api

_native = _pybevy.ecs
_ecs_dir = os.path.dirname(__file__)
_native.__name__ = __name__  # type: ignore
_native.__path__ = [_ecs_dir]  # type: ignore
_native.__package__ = __name__  # type: ignore
sys.modules[__name__] = _native  # type: ignore
_install_system_set_api(_native)

# Register Python submodules so `from pybevy.ecs import <submod>` works.
# The sys.modules replacement above breaks normal submodule discovery because
# the native module doesn't have Python submodule attributes. We fix this by
# eagerly importing each .py sibling and attaching it to the native module.
_package_name = "pybevy.ecs"
_lazy_modules = frozenset(("jax_ext",))
_optional_modules = frozenset(("numba_ext",))
for _fname in os.listdir(_ecs_dir):
    if _fname.startswith("_") or not _fname.endswith(".py"):
        continue
    _mod_name = _fname[:-3]
    if _mod_name in _lazy_modules:
        continue
    _qual = f"{_package_name}.{_mod_name}"
    if _qual not in sys.modules:
        _load_sibling_module(
            _native,
            _qual,
            os.path.join(_ecs_dir, _fname),
            optional=_mod_name in _optional_modules,
        )
