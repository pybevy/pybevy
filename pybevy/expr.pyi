"""Type stubs for lazy expression system."""

import numpy as np

class Expr:
    """Base class for lazy expression tree nodes."""

    op: str
    args: list

    def __init__(self, op: str, args: list) -> None: ...

    # Binary operators
    def __add__(self, other: Expr | float | int) -> Expr: ...
    def __radd__(self, other: Expr | float | int) -> Expr: ...
    def __sub__(self, other: Expr | float | int) -> Expr: ...
    def __rsub__(self, other: Expr | float | int) -> Expr: ...
    def __mul__(self, other: Expr | float | int) -> Expr: ...
    def __rmul__(self, other: Expr | float | int) -> Expr: ...
    def __truediv__(self, other: Expr | float | int) -> Expr: ...
    def __rtruediv__(self, other: Expr | float | int) -> Expr: ...
    def __pow__(self, other: Expr | float | int) -> Expr: ...
    def __rpow__(self, other: Expr | float | int) -> Expr: ...
    def __mod__(self, other: Expr | float | int) -> Expr: ...
    def __rmod__(self, other: Expr | float | int) -> Expr: ...

    # Unary operators
    def __neg__(self) -> Expr: ...

    # Comparison operators
    def __eq__(self, other: Expr | float | int) -> Expr: ...  # type: ignore[override]
    def __ne__(self, other: Expr | float | int) -> Expr: ...  # type: ignore[override]
    def __lt__(self, other: Expr | float | int) -> Expr: ...
    def __le__(self, other: Expr | float | int) -> Expr: ...
    def __gt__(self, other: Expr | float | int) -> Expr: ...
    def __ge__(self, other: Expr | float | int) -> Expr: ...

    # Logical operators
    def __and__(self, other: Expr | float | int) -> Expr: ...
    def __or__(self, other: Expr | float | int) -> Expr: ...
    def __invert__(self) -> Expr: ...

    # Math functions
    def sqrt(self) -> Expr: ...
    def abs(self) -> Expr: ...
    def min(self, other: Expr | float | int) -> Expr: ...
    def max(self, other: Expr | float | int) -> Expr: ...
    def clamp(
        self,
        min_val: Expr | float | int,
        max_val: Expr | float | int,
    ) -> Expr: ...

    # Trigonometric functions
    def sin(self) -> Expr: ...
    def cos(self) -> Expr: ...
    def tan(self) -> Expr: ...
    def asin(self) -> Expr: ...
    def acos(self) -> Expr: ...
    def atan(self) -> Expr: ...

    # Rounding functions
    def floor(self) -> Expr: ...
    def ceil(self) -> Expr: ...
    def round(self) -> Expr: ...

    # Exponential and logarithmic functions
    def exp(self) -> Expr: ...
    def ln(self) -> Expr: ...
    def log10(self) -> Expr: ...
    def log2(self) -> Expr: ...

    # Additional math operations
    def sign(self) -> Expr: ...
    def fract(self) -> Expr: ...
    def mod(self, other: Expr | float | int) -> Expr: ...
    def lerp(
        self,
        other: Expr | float | int,
        t: Expr | float | int,
    ) -> Expr: ...

    # Random functions
    def random(self) -> Expr: ...
    def random_range(
        self,
        min: Expr | float | int,
        max: Expr | float | int,
    ) -> Expr: ...

    # Conditional selection
    def where(
        self,
        true_value: Expr | float | int,
        false_value: Expr | float | int,
    ) -> Expr: ...

class FieldExpr(Expr):
    """Represents a scalar field in a View expression (e.g., intensity, range).

    Supports arithmetic operations, comparisons, and array-like indexing for Numba kernels.
    """

    component_id: int
    field_name: str
    offset: int
    field_type: str
    _parent_proxy: object | None

    def __init__(
        self, component_id: int, field_name: str, offset: int, field_type: str = "F32"
    ) -> None: ...

    def _set_parent(self, parent: object) -> None:
        """Internal: Set the parent proxy for assignment triggering."""

    # In-place operators (trigger immediate assignment)
    def __iadd__(self, other: Expr | float | int) -> FieldExpr: ...
    def __isub__(self, other: Expr | float | int) -> FieldExpr: ...
    def __imul__(self, other: Expr | float | int) -> FieldExpr: ...
    def __itruediv__(
        self, other: Expr | float | int
    ) -> FieldExpr: ...
    def __ipow__(self, other: Expr | float | int) -> FieldExpr: ...
    def __imod__(self, other: Expr | float | int) -> FieldExpr: ...

    # Assignment
    def set(self, value: Expr | float | int) -> None:
        """Assign a value or expression to this field."""

    # Array-like indexing for Numba kernels
    def __len__(self) -> int:
        """Get the number of elements in the field array."""

    def __getitem__(self, index: int) -> float:
        """Get the value at the given index (for Numba kernels)."""

    def __setitem__(self, index: int, value: float | int) -> None:
        """Set the value at the given index (for Numba kernels)."""

    # NumPy conversion (requires batch context)
    def to_numpy(self) -> np.ndarray:
        """Convert this field to a NumPy array (zero-copy view in batch context).

        Only valid within batch iteration context.
        Returns a NumPy array of shape (N,) for scalar fields.

        Raises:
            RuntimeError: If called outside batch iteration context
        """

    @property
    def len(self) -> int:
        """Get the length of the underlying data array."""

    def at_offset(self, length: int, dtype: str) -> FieldExpr:
        """Create a at_offset view with specified length and dtype.

        Args:
            length: Number of elements in the at_offset
            dtype: NumPy dtype string (e.g., "f4" for float32)

        Returns:
            A new FieldExpr representing the at_offset
        """

    def peek(self, index: int) -> float:
        """Peek at a value at the given index without bounds checking.

        Warning: This may read out of bounds if index >= len.
        """

class Vec3Expression:
    """Represents a Vec3 expression built from component-wise operations."""

    x: Expr
    y: Expr
    z: Expr

    def __init__(
        self,
        x_expr: Expr | float | int,
        y_expr: Expr | float | int,
        z_expr: Expr | float | int,
    ) -> None: ...
    def __add__(self, other: Vec3Expression | Vec3Expr) -> Vec3Expression: ...
    def __sub__(self, other: Vec3Expression | Vec3Expr) -> Vec3Expression: ...
    def __mul__(self, scalar: Expr | float | int) -> Vec3Expression: ...
    def __rmul__(self, scalar: Expr | float | int) -> Vec3Expression: ...
    def __truediv__(self, scalar: Expr | float | int) -> Vec3Expression: ...
    def __repr__(self) -> str: ...

class Vec3Expr:
    """Represents a Vec3 field in a View expression (e.g., translation, scale).

    Provides x, y, z properties that return FieldExpr instances for
    component-level access.
    """

    component_id: int
    base_field_name: str
    base_offset: int
    _parent_proxy: object | None

    def __init__(
        self,
        component_id: int,
        base_field_name: str,
        base_offset: int,
    ) -> None: ...

    def _set_parent(self, parent: object) -> None:
        """Internal: Set the parent proxy for assignment triggering."""

    @property
    def x(self) -> FieldExpr:
        """X component of the Vec3."""

    @x.setter
    def x(self, value: Expr | float | int) -> None: ...

    @property
    def y(self) -> FieldExpr:
        """Y component of the Vec3."""

    @y.setter
    def y(self, value: Expr | float | int) -> None: ...

    @property
    def z(self) -> FieldExpr:
        """Z component of the Vec3."""

    @z.setter
    def z(self, value: Expr | float | int) -> None: ...

    def set(self, vec3_expr: Vec3Expression | Vec3Expr) -> None:
        """Bulk assignment of all three components at once."""

class QuatExpr:
    """Represents a Quat field in a View expression (e.g., rotation).

    Provides x, y, z, w properties that return FieldExpr instances for
    component-level access.
    """

    component_id: int
    base_field_name: str
    base_offset: int
    _parent_proxy: object | None

    def __init__(
        self,
        component_id: int,
        base_field_name: str,
        base_offset: int,
    ) -> None: ...

    def _set_parent(self, parent: object) -> None:
        """Internal: Set the parent proxy for assignment triggering."""

    @property
    def x(self) -> FieldExpr:
        """X component of the quaternion."""

    @x.setter
    def x(self, value: Expr | float | int) -> None: ...

    @property
    def y(self) -> FieldExpr:
        """Y component of the quaternion."""

    @y.setter
    def y(self, value: Expr | float | int) -> None: ...

    @property
    def z(self) -> FieldExpr:
        """Z component of the quaternion."""

    @z.setter
    def z(self, value: Expr | float | int) -> None: ...

    @property
    def w(self) -> FieldExpr:
        """W (scalar) component of the quaternion."""

    @w.setter
    def w(self, value: Expr | float | int) -> None: ...

class Vec2Expr(Expr):
    """Represents a Vec2 field in a View expression.

    Provides x, y properties that return FieldExpr instances for
    component-level access.
    """

    @property
    def x(self) -> FieldExpr:
        """X component of the Vec2."""

    @property
    def y(self) -> FieldExpr:
        """Y component of the Vec2."""

# Module-level functional API
def const(value: float) -> Expr: ...
def where(
    condition: Expr,
    true_value: Expr | float | int,
    false_value: Expr | float | int,
) -> Expr: ...
def sin(x: Expr | float | int) -> Expr: ...
def cos(x: Expr | float | int) -> Expr: ...
def tan(x: Expr | float | int) -> Expr: ...
def asin(x: Expr | float | int) -> Expr: ...
def acos(x: Expr | float | int) -> Expr: ...
def atan(x: Expr | float | int) -> Expr: ...
def sqrt(x: Expr | float | int) -> Expr: ...
def abs(x: Expr | float | int) -> Expr: ...
def floor(x: Expr | float | int) -> Expr: ...
def ceil(x: Expr | float | int) -> Expr: ...
def round(x: Expr | float | int) -> Expr: ...
def min(a: Expr | float | int, b: Expr | float | int) -> Expr: ...
def max(a: Expr | float | int, b: Expr | float | int) -> Expr: ...
def clamp(
    x: Expr | float | int,
    min_val: Expr | float | int,
    max_val: Expr | float | int,
) -> Expr: ...
def exp(x: Expr | float | int) -> Expr: ...
def ln(x: Expr | float | int) -> Expr: ...
def log10(x: Expr | float | int) -> Expr: ...
def log2(x: Expr | float | int) -> Expr: ...
def sign(x: Expr | float | int) -> Expr: ...
def fract(x: Expr | float | int) -> Expr: ...
def mod(a: Expr | float | int, b: Expr | float | int) -> Expr: ...
def lerp(
    a: Expr | float | int,
    b: Expr | float | int,
    t: Expr | float | int,
) -> Expr: ...
