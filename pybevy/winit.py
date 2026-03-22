import sys

from . import _pybevy  # type: ignore

sys.modules[__name__] = _pybevy.winit  # type: ignore
