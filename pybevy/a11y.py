import sys

from . import _pybevy  # type: ignore

_pybevy.a11y.__name__ = __name__  # type: ignore
sys.modules[__name__] = _pybevy.a11y  # type: ignore
