import sys

from . import _pybevy  # type: ignore

_pybevy.post_process.__name__ = __name__  # type: ignore[attr-defined]
sys.modules[__name__] = _pybevy.post_process  # type: ignore[attr-defined]
