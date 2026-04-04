import sys

from . import _pybevy  # type: ignore

_pybevy.core_pipeline.__name__ = __name__  # type: ignore
sys.modules[__name__] = _pybevy.core_pipeline  # type: ignore
