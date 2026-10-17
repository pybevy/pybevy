import sys

from . import _pybevy  # type: ignore

_pybevy.shape.__name__ = __name__  # type: ignore
sys.modules[__name__] = _pybevy.shape  # type: ignore
