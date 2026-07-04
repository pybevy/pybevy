import sys

from . import _pybevy  # type: ignore

_pybevy.world_serialization.__name__ = __name__  # type: ignore
sys.modules[__name__] = _pybevy.world_serialization  # type: ignore
