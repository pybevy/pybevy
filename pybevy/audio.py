import sys

from . import _pybevy  # type: ignore

_pybevy.audio.__name__ = __name__  # type: ignore
sys.modules[__name__] = _pybevy.audio  # type: ignore
