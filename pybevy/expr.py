"""
Lazy expression system for View batch operations.

This module provides Python-side expression building that compiles to
native Rust bytecode for high-performance batch operations.
"""

from typing import Any, Union

_READ_ONLY_VIEW_ASSIGNMENT = (
    "Cannot assign through a read-only View column; use column_mut() with View[Mut[T]]"
)


def _assignment_parent(parent: Any) -> Any:  # noqa: ANN401
    if parent is None:
        raise RuntimeError(_READ_ONLY_VIEW_ASSIGNMENT)
    return parent


class Expr:
    """
    Base class for lazy expression tree nodes.

    When you write `pos.x + vel.x * 0.016`, this doesn't immediately calculate
    a value. Instead, it builds a tree of Expr objects that represent
    the computation. This tree is then passed to Rust and compiled to bytecode.
    """

    def __init__(self, op: str, args: list[Any]) -> None:
        """
        Create an expression node.

        Args:
            op: Operation type ("add", "mul", "field", "const", etc.)
            args: Child expressions or constant values
        """
        self.op = op
        self.args = args

    def __add__(self, other: Union["Expr", float, int]) -> "Expr":
        """Addition: self + other"""
        return Expr(op="add", args=[self, other])

    def __radd__(self, other: Union["Expr", float, int]) -> "Expr":
        """Right addition: other + self"""
        return Expr(op="add", args=[other, self])

    def __sub__(self, other: Union["Expr", float, int]) -> "Expr":
        """Subtraction: self - other"""
        return Expr(op="sub", args=[self, other])

    def __rsub__(self, other: Union["Expr", float, int]) -> "Expr":
        """Right subtraction: other - self"""
        return Expr(op="sub", args=[other, self])

    def __mul__(self, other: Union["Expr", float, int]) -> "Expr":
        """Multiplication: self * other"""
        return Expr(op="mul", args=[self, other])

    def __rmul__(self, other: Union["Expr", float, int]) -> "Expr":
        """Right multiplication: other * self"""
        return Expr(op="mul", args=[other, self])

    def __truediv__(self, other: Union["Expr", float, int]) -> "Expr":
        """Division: self / other"""
        return Expr(op="div", args=[self, other])

    def __rtruediv__(self, other: Union["Expr", float, int]) -> "Expr":
        """Right division: other / self"""
        return Expr(op="div", args=[other, self])

    def __pow__(self, other: Union["Expr", float, int]) -> "Expr":
        """Power: self ** other"""
        return Expr(op="pow", args=[self, other])

    def __rpow__(self, other: Union["Expr", float, int]) -> "Expr":
        """Right power: other ** self"""
        return Expr(op="pow", args=[other, self])

    def __mod__(self, other: Union["Expr", float, int]) -> "Expr":
        """Python modulo: the nonzero result has the divisor's sign."""
        return Expr(op="mod", args=[self, other])

    def __rmod__(self, other: Union["Expr", float, int]) -> "Expr":
        """Python modulo other % self, with the divisor's sign."""
        return Expr(op="mod", args=[other, self])

    def __neg__(self) -> "Expr":
        """Negation: -self"""
        return Expr(op="neg", args=[self])



    def __iadd__(self, other: Union["Expr", float, int]) -> "Expr":
        """In-place addition: self += other"""
        return self.__add__(other)

    def __isub__(self, other: Union["Expr", float, int]) -> "Expr":
        """In-place subtraction: self -= other"""
        return self.__sub__(other)

    def __imul__(self, other: Union["Expr", float, int]) -> "Expr":
        """In-place multiplication: self *= other"""
        return self.__mul__(other)

    def __itruediv__(self, other: Union["Expr", float, int]) -> "Expr":
        """In-place division: self /= other"""
        return self.__truediv__(other)

    def __ipow__(self, other: Union["Expr", float, int]) -> "Expr":
        """In-place power: self **= other"""
        return self.__pow__(other)

    def __imod__(self, other: Union["Expr", float, int]) -> "Expr":
        """In-place modulo: self %= other"""
        return self.__mod__(other)



    def sin(self) -> "Expr":
        """Sine function: sin(self)"""
        return Expr(op="sin", args=[self])

    def cos(self) -> "Expr":
        """Cosine function: cos(self)"""
        return Expr(op="cos", args=[self])

    def tan(self) -> "Expr":
        """Tangent function: tan(self)"""
        return Expr(op="tan", args=[self])

    def asin(self) -> "Expr":
        """Arcsine function: asin(self)"""
        return Expr(op="asin", args=[self])

    def acos(self) -> "Expr":
        """Arccosine function: acos(self)"""
        return Expr(op="acos", args=[self])

    def atan(self) -> "Expr":
        """Arctangent function: atan(self)"""
        return Expr(op="atan", args=[self])

    def sqrt(self) -> "Expr":
        """Square root: sqrt(self)"""
        return Expr(op="sqrt", args=[self])

    def abs(self) -> "Expr":
        """Absolute value: abs(self)"""
        return Expr(op="abs", args=[self])

    def floor(self) -> "Expr":
        """Floor function: floor(self)"""
        return Expr(op="floor", args=[self])

    def ceil(self) -> "Expr":
        """Ceiling function: ceil(self)"""
        return Expr(op="ceil", args=[self])

    def round(self) -> "Expr":
        """Round to the nearest integer, breaking ties toward even."""
        return Expr(op="round", args=[self])

    def min(self, other: Union["Expr", float, int]) -> "Expr":
        """Element-wise minimum, propagating NaN."""
        return Expr(op="min", args=[self, other])

    def max(self, other: Union["Expr", float, int]) -> "Expr":
        """Element-wise maximum, propagating NaN."""
        return Expr(op="max", args=[self, other])

    def clamp(
        self,
        min_val: Union["Expr", float, int],
        max_val: Union["Expr", float, int],
    ) -> "Expr":
        """Clip between bounds; NaN propagates and reversed bounds return max_val."""
        return Expr(op="clamp", args=[self, min_val, max_val])

    def exp(self) -> "Expr":
        """Exponential function: e^self"""
        return Expr(op="exp", args=[self])

    def ln(self) -> "Expr":
        """Natural logarithm: ln(self)"""
        return Expr(op="ln", args=[self])

    def log10(self) -> "Expr":
        """Base-10 logarithm: log10(self)"""
        return Expr(op="log10", args=[self])

    def log2(self) -> "Expr":
        """Base-2 logarithm: log2(self)"""
        return Expr(op="log2", args=[self])

    def sign(self) -> "Expr":
        """Return -1.0, 0.0, or 1.0 by sign, propagating NaN."""
        return Expr(op="sign", args=[self])

    def fract(self) -> "Expr":
        """Signed fractional part: fract(self) = self - trunc(self)."""
        return Expr(op="fract", args=[self])

    def mod(self, other: Union["Expr", float, int]) -> "Expr":
        """Python modulo: the nonzero result has the divisor's sign."""
        return Expr(op="mod", args=[self, other])

    def lerp(
        self,
        other: Union["Expr", float, int],
        t: Union["Expr", float, int],
    ) -> "Expr":
        """Linear interpolation: lerp(self, other, t) = self + t * (other - self)"""
        return Expr(op="lerp", args=[self, other, t])

    def random(self) -> "Expr":
        """Generate a random float in [0.0, 1.0).

        The random value is deterministic based on the entity, so the same entity
        will always get the same random value. This is useful for procedural generation.

        Example:
            # Randomize height field
            view[Transform].translation.y.set(y.random() * 10.0)
        """
        return Expr(op="random", args=[])

    def random_range(
        self,
        min: Union["Expr", float, int],
        max: Union["Expr", float, int],
    ) -> "Expr":
        """Generate a random float in [min, max).

        The random value is deterministic based on the entity, so the same entity
        will always get the same random value. This is useful for procedural generation.

        Example:
            # Randomize scale between 0.5 and 2.0
            view[Transform].scale.x.set(x.random_range(0.5, 2.0))
        """
        return Expr(op="random_range", args=[min, max])

    def where(
        self,
        true_value: Union["Expr", float, int],
        false_value: Union["Expr", float, int],
    ) -> "Expr":
        """Conditional selection: return true_value if self is true, else false_value.

        This allows per-entity branching based on boolean conditions.

        Example:
            # Clamp negative values to zero
            y.set((y < 0.0).where(0.0, y))

            # Apply bonus only to high-value entities
            new_val = val + (val > 100.0).where(bonus, 0.0)
        """
        return Expr(op="where", args=[self, true_value, false_value])

    def __eq__(self, other: Union["Expr", float, int]) -> "Expr":  # type: ignore[override]
        """Equal to: self == other"""
        return Expr(op="eq", args=[self, other])

    def __ne__(self, other: Union["Expr", float, int]) -> "Expr":  # type: ignore[override]
        """Not equal to: self != other"""
        return Expr(op="ne", args=[self, other])

    def __lt__(self, other: Union["Expr", float, int]) -> "Expr":
        """Less than: self < other"""
        return Expr(op="lt", args=[self, other])

    def __le__(self, other: Union["Expr", float, int]) -> "Expr":
        """Less than or equal: self <= other"""
        return Expr(op="le", args=[self, other])

    def __gt__(self, other: Union["Expr", float, int]) -> "Expr":
        """Greater than: self > other"""
        return Expr(op="gt", args=[self, other])

    def __ge__(self, other: Union["Expr", float, int]) -> "Expr":
        """Greater than or equal: self >= other"""
        return Expr(op="ge", args=[self, other])

    def __and__(self, other: Union["Expr", float, int]) -> "Expr":
        """Logical AND: self & other

        Combines two boolean expressions with AND logic.
        Both operands must be boolean expressions (e.g., from comparisons).

        Example:
            >>> health_low = health < 20.0
            >>> shield_down = shield <= 0.0
            >>> critical = health_low & shield_down  # Both must be true
        """
        return Expr(op="and", args=[self, other])

    def __or__(self, other: Union["Expr", float, int]) -> "Expr":
        """Logical OR: self | other

        Combines two boolean expressions with OR logic.
        At least one operand must be true for result to be true.

        Example:
            >>> health_low = health < 20.0
            >>> shield_down = shield <= 0.0
            >>> vulnerable = health_low | shield_down  # Either is dangerous
        """
        return Expr(op="or", args=[self, other])

    def __invert__(self) -> "Expr":
        """Logical NOT: ~self

        Inverts a boolean expression.

        Example:
            >>> is_alive = health > 0.0
            >>> is_dead = ~is_alive  # Logical negation
        """
        return Expr(op="not", args=[self])

    def __repr__(self) -> str:
        """String representation for debugging"""
        if self.op == "const":
            return f"Const({self.args[0]})"
        if self.op == "field":
            return f"Field({self.args[1]})"  # args[1] is field_name
        if len(self.args) == 2:
            return f"({self.args[0]} {self.op} {self.args[1]})"
        if len(self.args) == 1:
            return f"{self.op}({self.args[0]})"
        return f"{self.op}{self.args}"


class FieldExpr(Expr):
    """
    Represents a component field in a lazy expression (e.g., pos.x, vel.y).

    This is the "leaf" node in expression trees - it refers to actual data
    that will be loaded from component memory during execution.
    """

    def __init__(self, component_id: int, field_name: str, offset: int, field_type: str = "F32") -> None:
        """
        Create a field proxy.

        Args:
            component_id: Bevy's ComponentId (internal identifier)
            field_name: Human-readable field name for debugging ("x", "y", etc.)
            offset: Byte offset of this field within the component struct
            field_type: Type of the field (F32, F64, I32, I64, U8, U32, U64, Bool)
        """
        # Store as "field" operation with component_id, field_name, offset, field_type
        super().__init__(op="field", args=[component_id, field_name, offset, field_type])
        self.component_id = component_id
        self.field_name = field_name
        self.offset = offset
        self.field_type = field_type
        self._parent_proxy: Any = None  # Will be set by Vec3Expr

    def _set_parent(self, parent: Any) -> None:  # noqa: ANN401  # Internal method, parent can be various proxy types
        """Internal: Set the parent proxy for assignment triggering."""
        self._parent_proxy = parent


    def __iadd__(self, other: Union["Expr", float, int]) -> "FieldExpr":
        """In-place addition: self += other.

        Returns the expression; the property setter handles execution.
        (Don't call self.set() here - Python will call the setter with the return value)
        """
        return self.__add__(other)

    def __isub__(self, other: Union["Expr", float, int]) -> "FieldExpr":
        """In-place subtraction: self -= other."""
        return self.__sub__(other)

    def __imul__(self, other: Union["Expr", float, int]) -> "FieldExpr":
        """In-place multiplication: self *= other."""
        return self.__mul__(other)

    def __itruediv__(
        self, other: Union["Expr", float, int]
    ) -> "FieldExpr":
        """In-place division: self /= other."""
        return self.__truediv__(other)

    def __ipow__(self, other: Union["Expr", float, int]) -> "FieldExpr":
        """In-place power: self **= other."""
        return self.__pow__(other)

    def __imod__(self, other: Union["Expr", float, int]) -> "FieldExpr":
        """In-place modulo: self %= other."""
        return self.__mod__(other)

    def set(self, value: Union["Expr", float, int]) -> None:
        """
        Set this field to the result of an expression.

        This triggers compilation and execution of the expression,
        writing the result back to all entities in the view.

        Example:
            # Set all x values to random
            transform.translation.x.set(transform.translation.x.random())

            # Set y to computed expression
            y.set(y * 2.0 + 1.0)
        """
        parent = _assignment_parent(self._parent_proxy)
        parent._trigger_assignment(self.field_name, value)

    def __repr__(self) -> str:
        return f"Field({self.field_name})"


class Vec3Expression:
    """Vec3 expression built from component-wise operations, allowing bulk
    assignments instead of three separate field writes:

        transform.translation = transform.translation + velocity
    """

    def __init__(
        self,
        x_expr: Expr | float | int,
        y_expr: Expr | float | int,
        z_expr: Expr | float | int,
    ) -> None:
        """Create a Vec3 expression from component expressions."""
        self.x = (
            x_expr
            if isinstance(x_expr, Expr)
            else Expr("const", [x_expr])
        )
        self.y = (
            y_expr
            if isinstance(y_expr, Expr)
            else Expr("const", [y_expr])
        )
        self.z = (
            z_expr
            if isinstance(z_expr, Expr)
            else Expr("const", [z_expr])
        )

    def __add__(
        self, other: Union["Vec3Expression", "Vec3Expr"]
    ) -> "Vec3Expression":
        """Vec3 + Vec3"""
        return Vec3Expression(
            x_expr=self.x + other.x,
            y_expr=self.y + other.y,
            z_expr=self.z + other.z,
        )

    def __sub__(
        self, other: Union["Vec3Expression", "Vec3Expr"]
    ) -> "Vec3Expression":
        """Vec3 - Vec3"""
        return Vec3Expression(
            x_expr=self.x - other.x,
            y_expr=self.y - other.y,
            z_expr=self.z - other.z,
        )

    def __mul__(self, scalar: Expr | float | int) -> "Vec3Expression":
        """Vec3 * scalar"""
        return Vec3Expression(
            x_expr=self.x * scalar,
            y_expr=self.y * scalar,
            z_expr=self.z * scalar,
        )

    def __rmul__(self, scalar: Expr | float | int) -> "Vec3Expression":
        """scalar * Vec3"""
        return self.__mul__(scalar)

    def __truediv__(self, scalar: Expr | float | int) -> "Vec3Expression":
        """Vec3 / scalar"""
        return Vec3Expression(
            x_expr=self.x / scalar,
            y_expr=self.y / scalar,
            z_expr=self.z / scalar,
        )

    def __repr__(self) -> str:
        return f"Vec3({self.x}, {self.y}, {self.z})"


class Vec3Expr:
    """
    Proxy for Vec3 fields (translation, scale) that provides x/y/z access.

    When you access `transform.translation`, you get a Vec3Expr.
    Then `translation.x` returns a FieldExpr for the x component.

    Also supports Vec3 arithmetic operations for bulk assignment:
        transform.translation = transform.translation + velocity
    """

    def __init__(
        self, component_id: int, base_field_name: str, base_offset: int
    ) -> None:
        """
        Create a Vec3 field proxy.

        Args:
            component_id: Bevy's ComponentId
            base_field_name: The Vec3 field name ("translation", "scale")
            base_offset: Byte offset of the Vec3 within the component
        """
        self.component_id = component_id
        self.base_field_name = base_field_name
        self.base_offset = base_offset
        # Store reference to the parent ViewColMut for triggering assignments
        self._parent_proxy: Any = None

    def _set_parent(self, parent: Any) -> None:  # noqa: ANN401  # Internal method, parent can be various proxy types
        """Internal: Set the parent proxy for assignment triggering."""
        self._parent_proxy = parent


    def __add__(
        self, other: Union["Vec3Expr", "Vec3Expression"]
    ) -> "Vec3Expression":
        """Vec3 addition: self + other"""
        return Vec3Expression(
            x_expr=self.x + other.x,
            y_expr=self.y + other.y,
            z_expr=self.z + other.z,
        )

    def __sub__(
        self, other: Union["Vec3Expr", "Vec3Expression"]
    ) -> "Vec3Expression":
        """Vec3 subtraction: self - other"""
        return Vec3Expression(
            x_expr=self.x - other.x,
            y_expr=self.y - other.y,
            z_expr=self.z - other.z,
        )

    def __mul__(self, scalar: Expr | float | int) -> "Vec3Expression":
        """Vec3 scalar multiplication: self * scalar"""
        return Vec3Expression(
            x_expr=self.x * scalar,
            y_expr=self.y * scalar,
            z_expr=self.z * scalar,
        )

    def __rmul__(self, scalar: Expr | float | int) -> "Vec3Expression":
        """Vec3 scalar multiplication: scalar * self"""
        return self.__mul__(scalar)

    def __truediv__(self, scalar: Expr | float | int) -> "Vec3Expression":
        """Vec3 scalar division: self / scalar"""
        return Vec3Expression(
            x_expr=self.x / scalar,
            y_expr=self.y / scalar,
            z_expr=self.z / scalar,
        )

    @property
    def x(self) -> FieldExpr:
        """Access the x component (first f32 in Vec3)"""
        proxy = FieldExpr(
            self.component_id,
            f"{self.base_field_name}.x",
            self.base_offset + 0,  # x is at offset 0
        )
        proxy._set_parent(self._parent_proxy)
        return proxy

    @x.setter
    def x(self, value: Expr | float | int) -> None:
        """Set the x component using an expression or constant."""
        parent = _assignment_parent(self._parent_proxy)
        parent._trigger_assignment(f"{self.base_field_name}.x", value)

    @property
    def y(self) -> FieldExpr:
        """Access the y component (second f32 in Vec3)"""
        proxy = FieldExpr(
            self.component_id,
            f"{self.base_field_name}.y",
            self.base_offset + 4,  # y is at offset 4 (after 1 f32)
        )
        proxy._set_parent(self._parent_proxy)
        return proxy

    @y.setter
    def y(self, value: Expr | float | int) -> None:
        """Set the y component using an expression or constant."""
        parent = _assignment_parent(self._parent_proxy)
        parent._trigger_assignment(f"{self.base_field_name}.y", value)

    @property
    def z(self) -> FieldExpr:
        """Access the z component (third f32 in Vec3)"""
        proxy = FieldExpr(
            self.component_id,
            f"{self.base_field_name}.z",
            self.base_offset + 8,  # z is at offset 8 (after 2 f32s)
        )
        proxy._set_parent(self._parent_proxy)
        return proxy

    @z.setter
    def z(self, value: Expr | float | int) -> None:
        """Set the z component using an expression or constant."""
        parent = _assignment_parent(self._parent_proxy)
        parent._trigger_assignment(f"{self.base_field_name}.z", value)

    def set(self, vec3_expr: Union["Vec3Expression", "Vec3Expr"]) -> None:
        """Bulk assignment of all three components at once (x, then y, then z):

            transform.translation.set(transform.translation + velocity)
        """
        parent = _assignment_parent(self._parent_proxy)
        parent._trigger_assignment(f"{self.base_field_name}.x", vec3_expr.x)
        parent._trigger_assignment(f"{self.base_field_name}.y", vec3_expr.y)
        parent._trigger_assignment(f"{self.base_field_name}.z", vec3_expr.z)

    def __repr__(self) -> str:
        return f"Vec3Field({self.base_field_name})"


class Vec2Expr:
    """
    Proxy for Vec2 fields that provides x/y access.

    When you access a Vec2 field in a View, you get a Vec2Expr.
    Then `.x` / `.y` return FieldExpr instances for the individual components.
    """

    def __init__(
        self, component_id: int, base_field_name: str, base_offset: int
    ) -> None:
        self.component_id = component_id
        self.base_field_name = base_field_name
        self.base_offset = base_offset
        self._parent_proxy: Any = None

    def _set_parent(self, parent: Any) -> None:  # noqa: ANN401
        """Internal: Set the parent proxy for assignment triggering."""
        self._parent_proxy = parent

    @property
    def x(self) -> FieldExpr:
        """Access the x component (first f32 in Vec2)"""
        proxy = FieldExpr(
            self.component_id,
            f"{self.base_field_name}.x",
            self.base_offset + 0,
        )
        proxy._set_parent(self._parent_proxy)
        return proxy

    @x.setter
    def x(self, value: Expr | float | int) -> None:
        """Set the x component using an expression or constant."""
        parent = _assignment_parent(self._parent_proxy)
        parent._trigger_assignment(f"{self.base_field_name}.x", value)

    @property
    def y(self) -> FieldExpr:
        """Access the y component (second f32 in Vec2)"""
        proxy = FieldExpr(
            self.component_id,
            f"{self.base_field_name}.y",
            self.base_offset + 4,
        )
        proxy._set_parent(self._parent_proxy)
        return proxy

    @y.setter
    def y(self, value: Expr | float | int) -> None:
        """Set the y component using an expression or constant."""
        parent = _assignment_parent(self._parent_proxy)
        parent._trigger_assignment(f"{self.base_field_name}.y", value)

    def __repr__(self) -> str:
        return f"Vec2Field({self.base_field_name})"


class QuatExpr:
    """
    Proxy for Quat fields (rotation) that provides x/y/z/w access.

    When you access `transform.rotation`, you get a QuatExpr.
    Then `rotation.x` returns a FieldExpr for the x component.
    """

    def __init__(
        self, component_id: int, base_field_name: str, base_offset: int
    ) -> None:
        """
        Create a Quat field proxy.

        Args:
            component_id: Bevy's ComponentId
            base_field_name: The Quat field name ("rotation")
            base_offset: Byte offset of the Quat within the component
        """
        self.component_id = component_id
        self.base_field_name = base_field_name
        self.base_offset = base_offset
        # Store reference to the parent ViewColMut for triggering assignments
        self._parent_proxy: Any = None

    def _set_parent(self, parent: Any) -> None:  # noqa: ANN401  # Internal method, parent can be various proxy types
        """Internal: Set the parent proxy for assignment triggering."""
        self._parent_proxy = parent

    @property
    def x(self) -> FieldExpr:
        """Access the x component (first f32 in Quat)"""
        proxy = FieldExpr(
            self.component_id, f"{self.base_field_name}.x", self.base_offset + 0
        )
        proxy._set_parent(self._parent_proxy)
        return proxy

    @x.setter
    def x(self, value: Expr | float | int) -> None:
        """Set the x component using an expression or constant."""
        parent = _assignment_parent(self._parent_proxy)
        parent._trigger_assignment(f"{self.base_field_name}.x", value)

    @property
    def y(self) -> FieldExpr:
        """Access the y component (second f32 in Quat)"""
        proxy = FieldExpr(
            self.component_id, f"{self.base_field_name}.y", self.base_offset + 4
        )
        proxy._set_parent(self._parent_proxy)
        return proxy

    @y.setter
    def y(self, value: Expr | float | int) -> None:
        """Set the y component using an expression or constant."""
        parent = _assignment_parent(self._parent_proxy)
        parent._trigger_assignment(f"{self.base_field_name}.y", value)

    @property
    def z(self) -> FieldExpr:
        """Access the z component (third f32 in Quat)"""
        proxy = FieldExpr(
            self.component_id, f"{self.base_field_name}.z", self.base_offset + 8
        )
        proxy._set_parent(self._parent_proxy)
        return proxy

    @z.setter
    def z(self, value: Expr | float | int) -> None:
        """Set the z component using an expression or constant."""
        parent = _assignment_parent(self._parent_proxy)
        parent._trigger_assignment(f"{self.base_field_name}.z", value)

    @property
    def w(self) -> FieldExpr:
        """Access the w component (fourth f32 in Quat)"""
        proxy = FieldExpr(
            self.component_id, f"{self.base_field_name}.w", self.base_offset + 12
        )
        proxy._set_parent(self._parent_proxy)
        return proxy

    @w.setter
    def w(self, value: Expr | float | int) -> None:
        """Set the w component using an expression or constant."""
        parent = _assignment_parent(self._parent_proxy)
        parent._trigger_assignment(f"{self.base_field_name}.w", value)

    def __repr__(self) -> str:
        return f"QuatField({self.base_field_name})"


def const(value: float) -> Expr:
    """Create a constant expression node.

    Usually not needed: numeric literals are converted automatically.
    """
    return Expr(op="const", args=[value])


def where(
    condition: Expr,
    true_value: Expr | float | int,
    false_value: Expr | float | int,
) -> Expr:
    """Conditional selection: choose between two values based on a boolean condition.

    Enables per-entity branching: for each entity, ``true_value`` is selected when
    ``condition`` is true, otherwise ``false_value``.

    Examples:
        >>> # Clamp negative values to zero
        >>> y = where(y < 0.0, 0.0, y)
        >>>
        >>> # Apply damage only if health > 0
        >>> new_health = where(health > 0.0, health - damage, 0.0)
        >>>
        >>> # Clamp to range (can chain where calls)
        >>> clamped = where(x > max_val, max_val, where(x < min_val, min_val, x))
        >>>
        >>> # Complex game logic
        >>> damage = where(
        ...     (health > 0.0) & (shield <= 0.0),  # Alive but unshielded
        ...     attack_power,  # Take full damage
        ...     0.0  # Either dead or shielded
        ... )
    """
    return Expr(op="where", args=[condition, true_value, false_value])


def _ensure_expr(value: Expr | float | int) -> Expr:
    """Convert a value to a Expr if it isn't already."""
    return value if isinstance(value, Expr) else const(value)


def sin(x: Expr | float | int) -> Expr:
    """Sine function."""
    return _ensure_expr(x).sin()


def cos(x: Expr | float | int) -> Expr:
    """Cosine function."""
    return _ensure_expr(x).cos()


def tan(x: Expr | float | int) -> Expr:
    """Tangent function."""
    return _ensure_expr(x).tan()


def asin(x: Expr | float | int) -> Expr:
    """Arcsine function."""
    return _ensure_expr(x).asin()


def acos(x: Expr | float | int) -> Expr:
    """Arccosine function."""
    return _ensure_expr(x).acos()


def atan(x: Expr | float | int) -> Expr:
    """Arctangent function."""
    return _ensure_expr(x).atan()


def sqrt(x: Expr | float | int) -> Expr:
    """Square root function."""
    return _ensure_expr(x).sqrt()


def abs(x: Expr | float | int) -> Expr:
    """Absolute value function."""
    return _ensure_expr(x).abs()


def floor(x: Expr | float | int) -> Expr:
    """Floor function."""
    return _ensure_expr(x).floor()


def ceil(x: Expr | float | int) -> Expr:
    """Ceiling function."""
    return _ensure_expr(x).ceil()


def round(x: Expr | float | int) -> Expr:
    """Round to the nearest integer, breaking ties toward even."""
    return _ensure_expr(x).round()


def min(a: Expr | float | int, b: Expr | float | int) -> Expr:
    """Element-wise minimum, propagating NaN."""
    return _ensure_expr(a).min(b)


def max(a: Expr | float | int, b: Expr | float | int) -> Expr:
    """Element-wise maximum, propagating NaN."""
    return _ensure_expr(a).max(b)


def clamp(
    x: Expr | float | int,
    min_val: Expr | float | int,
    max_val: Expr | float | int,
) -> Expr:
    """Clip between bounds; NaN propagates and reversed bounds return max_val."""
    return _ensure_expr(x).clamp(min_val, max_val)


def exp(x: Expr | float | int) -> Expr:
    """Exponential function: e^x."""
    return _ensure_expr(x).exp()


def ln(x: Expr | float | int) -> Expr:
    """Natural logarithm."""
    return _ensure_expr(x).ln()


def log10(x: Expr | float | int) -> Expr:
    """Base-10 logarithm."""
    return _ensure_expr(x).log10()


def log2(x: Expr | float | int) -> Expr:
    """Base-2 logarithm."""
    return _ensure_expr(x).log2()


def sign(x: Expr | float | int) -> Expr:
    """Return -1.0, 0.0, or 1.0 by sign, propagating NaN."""
    return _ensure_expr(x).sign()


def fract(x: Expr | float | int) -> Expr:
    """Signed fractional part: fract(x) = x - trunc(x)."""
    return _ensure_expr(x).fract()


def mod(a: Expr | float | int, b: Expr | float | int) -> Expr:
    """Python modulo; a nonzero result has the divisor's sign."""
    return _ensure_expr(a).mod(b)


def lerp(
    a: Expr | float | int,
    b: Expr | float | int,
    t: Expr | float | int,
) -> Expr:
    """Linear interpolation: lerp(a, b, t) = a + t * (b - a)."""
    return _ensure_expr(a).lerp(b, t)
