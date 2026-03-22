"""
JAX extension for ViewColumn interop.

This module teaches JAX how to consume ViewColumn handles via the
__jax_array__ protocol, and provides write-back via from_jax().

Importing this module activates JAX support:
  - ViewColumn gains .to_jax() and .from_jax() methods
  - ViewColumn implements __jax_array__ for transparent @jax.jit input conversion
  - Vec3ViewColumn and QuatViewColumn are registered as JAX pytrees
"""

from types import SimpleNamespace

try:
    import jax  # type: ignore[import-untyped]
    import jax.numpy as jnp  # type: ignore[import-untyped]
except ImportError as err:
    raise ImportError(
        "JAX is required for ViewColumn JAX interop.\n"
        "Install with: pip install jax jaxlib\n"
        "Or: poetry install --extras jax"
    ) from err

import numpy as np

from pybevy.ecs import ViewColumn
from pybevy.ecs.view_accessors import QuatViewColumn, Vec3ViewColumn

JaxArray = jax.Array

_DTYPE_MAP: dict[str, np.dtype[np.generic]] = {
    "f4": np.dtype(np.float32),
    "f8": np.dtype(np.float64),
    "i4": np.dtype(np.int32),
    "i8": np.dtype(np.int64),
}


# ============================================================================
# ViewColumn → JAX
# ============================================================================


def _to_jax(self: ViewColumn) -> JaxArray:
    """Convert ViewColumn to a JAX array (copy).

    Uses to_contiguous_bytes() for dtype-preserving bulk read,
    then wraps via numpy (zero-copy numpy→JAX on CPU).
    """
    raw = self.to_contiguous_bytes()
    np_arr = np.frombuffer(raw, dtype=_DTYPE_MAP[self.dtype])
    return jnp.array(np_arr)


# ============================================================================
# JAX → ViewColumn
# ============================================================================


def _from_jax(self: ViewColumn, arr: JaxArray) -> None:
    """Write JAX array contents back into ECS storage.

    Converts to numpy first (zero-copy on CPU), then uses
    write_from_buffer() for stride-aware bulk write.
    """
    np_arr = np.asarray(arr, dtype=_DTYPE_MAP[self.dtype])
    self.write_from_buffer(np_arr.tobytes())


# ============================================================================
# Monkey-patch ViewColumn
# ============================================================================

ViewColumn.to_jax = _to_jax  # type: ignore[method-assign,attr-defined]
ViewColumn.from_jax = _from_jax  # type: ignore[method-assign,attr-defined]


# ============================================================================
# Vec3ViewColumn / QuatViewColumn write-back
# ============================================================================


def _vec3_from_jax(
    self: Vec3ViewColumn,
    x_or_obj: "JaxArray | SimpleNamespace",
    y: "JaxArray | None" = None,
    z: "JaxArray | None" = None,
) -> None:
    """Write JAX arrays back into the 3 underlying ViewColumns.

    Accepts either:
      - vec3col.from_jax(result)  where result has .x, .y, .z attributes
      - vec3col.from_jax(x, y, z) as 3 separate arrays
    """
    if y is not None and z is not None:
        _from_jax(self.x, x_or_obj)  # type: ignore[arg-type]
        _from_jax(self.y, y)
        _from_jax(self.z, z)
    else:
        _from_jax(self.x, x_or_obj.x)  # type: ignore[union-attr]
        _from_jax(self.y, x_or_obj.y)  # type: ignore[union-attr]
        _from_jax(self.z, x_or_obj.z)  # type: ignore[union-attr]


def _quat_from_jax(
    self: QuatViewColumn,
    x_or_obj: "JaxArray | SimpleNamespace",
    y: "JaxArray | None" = None,
    z: "JaxArray | None" = None,
    w: "JaxArray | None" = None,
) -> None:
    """Write JAX arrays back into the 4 underlying ViewColumns.

    Accepts either:
      - quatcol.from_jax(result)  where result has .x, .y, .z, .w attributes
      - quatcol.from_jax(x, y, z, w) as 4 separate arrays
    """
    if y is not None and z is not None and w is not None:
        _from_jax(self.x, x_or_obj)  # type: ignore[arg-type]
        _from_jax(self.y, y)
        _from_jax(self.z, z)
        _from_jax(self.w, w)
    else:
        _from_jax(self.x, x_or_obj.x)  # type: ignore[union-attr]
        _from_jax(self.y, x_or_obj.y)  # type: ignore[union-attr]
        _from_jax(self.z, x_or_obj.z)  # type: ignore[union-attr]
        _from_jax(self.w, x_or_obj.w)  # type: ignore[union-attr]


Vec3ViewColumn.from_jax = _vec3_from_jax  # type: ignore[attr-defined]
QuatViewColumn.from_jax = _quat_from_jax  # type: ignore[attr-defined]


# ============================================================================
# JAX pytree registration
# ============================================================================

# ViewColumn as pytree leaf — JAX auto-converts via to_jax() at JIT boundary.
# This replaces the deprecated __jax_array__ protocol (removed in JAX 0.9+).


def _viewcolumn_flatten(v: ViewColumn) -> tuple[list[JaxArray], None]:
    return [_to_jax(v)], None


def _viewcolumn_unflatten(aux: None, children: list[JaxArray]) -> JaxArray:
    return children[0]


def _vec3_flatten(v: Vec3ViewColumn) -> tuple[list[JaxArray], None]:
    return [_to_jax(v.x), _to_jax(v.y), _to_jax(v.z)], None


def _vec3_unflatten(
    aux: None, children: list[JaxArray],
) -> SimpleNamespace:
    return SimpleNamespace(x=children[0], y=children[1], z=children[2])


def _quat_flatten(v: QuatViewColumn) -> tuple[list[JaxArray], None]:
    return [_to_jax(v.x), _to_jax(v.y), _to_jax(v.z), _to_jax(v.w)], None


def _quat_unflatten(
    aux: None, children: list[JaxArray],
) -> SimpleNamespace:
    return SimpleNamespace(
        x=children[0], y=children[1], z=children[2], w=children[3],
    )


jax.tree_util.register_pytree_node(ViewColumn, _viewcolumn_flatten, _viewcolumn_unflatten)  # type: ignore[arg-type]
jax.tree_util.register_pytree_node(Vec3ViewColumn, _vec3_flatten, _vec3_unflatten)  # type: ignore[arg-type]
jax.tree_util.register_pytree_node(QuatViewColumn, _quat_flatten, _quat_unflatten)  # type: ignore[arg-type]

# SimpleNamespace is used as return type from Vec3/Quat unflatten and from @jax.jit
# functions that return structured results. Register it so JAX can trace through it.


def _sns_flatten(ns: SimpleNamespace) -> tuple[list[object], list[str]]:
    keys = sorted(ns.__dict__.keys())
    return [ns.__dict__[k] for k in keys], keys


def _sns_unflatten(keys: list[str], children: list[object]) -> SimpleNamespace:
    return SimpleNamespace(**dict(zip(keys, children, strict=True)))


jax.tree_util.register_pytree_node(SimpleNamespace, _sns_flatten, _sns_unflatten)  # type: ignore[arg-type,type-var]


__all__ = [
    "QuatViewColumn",
    "Vec3ViewColumn",
    "ViewColumn",
]
