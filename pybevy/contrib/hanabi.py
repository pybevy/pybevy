import sys

from .. import _pybevy  # type: ignore

try:
    _native = _pybevy.contrib.hanabi  # type: ignore[attr-defined]
except AttributeError:  # pragma: no cover - build-time feature gate
    raise ImportError(
        "this pybevy build does not include GPU particle support "
        "(built without the 'hanabi' cargo feature)"
    ) from None

_native.__name__ = __name__  # type: ignore
sys.modules[__name__] = _native  # type: ignore
