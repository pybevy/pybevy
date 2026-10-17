import sys

from . import _pybevy  # type: ignore

_pybevy.input_focus.__name__ = __name__  # type: ignore
sys.modules[__name__] = _pybevy.input_focus  # type: ignore
