import sys

from . import _pybevy  # type: ignore

_pybevy.render.__name__ = __name__  # type: ignore
sys.modules[__name__] = _pybevy.render  # type: ignore
