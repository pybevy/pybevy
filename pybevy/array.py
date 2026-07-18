import sys

from . import _pybevy

_pybevy.array.__name__ = __name__
sys.modules[__name__] = _pybevy.array
