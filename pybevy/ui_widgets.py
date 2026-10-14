import sys

from . import _pybevy  # type: ignore

_pybevy.ui_widgets.__name__ = __name__  # type: ignore
sys.modules[__name__] = _pybevy.ui_widgets  # type: ignore
