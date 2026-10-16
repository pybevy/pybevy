"""Scalar annotations shared by storage bindings and shader records."""

from typing import Annotated

# Ordinary Python integers, with an unsigned WGSL storage representation.
U32 = Annotated[int, "u32"]
