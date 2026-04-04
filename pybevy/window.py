import sys

from . import _pybevy  # type: ignore

_pybevy.window.__name__ = __name__  # type: ignore
sys.modules[__name__] = _pybevy.window  # type: ignore
