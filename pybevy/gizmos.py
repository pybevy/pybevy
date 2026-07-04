import sys

from . import _pybevy  # type: ignore

_pybevy.gizmos.__name__ = __name__  # type: ignore
sys.modules[__name__] = _pybevy.gizmos  # type: ignore
