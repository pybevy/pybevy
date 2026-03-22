import sys

import pybevy._pybevy as _pybevy  # type: ignore

sys.modules[__name__] = _pybevy.render_readback  # type: ignore
