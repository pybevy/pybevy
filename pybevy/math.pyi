from typing import ClassVar, overload

class FloatOrd:
    """A float with Bevy's total ordering and hashing semantics.

    NaN values compare equal to each other and sort before every non-NaN value.
    Positive and negative zero compare equal and have the same hash.
    """

    def __init__(self, value: float) -> None: ...
    @property
    def value(self) -> float: ...
    def __float__(self) -> float: ...
    def __neg__(self) -> FloatOrd: ...
    def __eq__(self, other: object) -> bool: ...
    def __lt__(self, other: FloatOrd) -> bool: ...
    def __le__(self, other: FloatOrd) -> bool: ...
    def __gt__(self, other: FloatOrd) -> bool: ...
    def __ge__(self, other: FloatOrd) -> bool: ...
    def __hash__(self) -> int: ...

    def __copy__(self) -> FloatOrd:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> FloatOrd:
        """Return an independent copy; the value holds no Python objects."""

class Vec3:
    """A 3-dimensional vector class providing common vector operations.

    This class supports operations like dot product, cross product, normalization,
    and various utility methods for vector manipulation.
    """

    x: float
    y: float
    z: float

    ZERO: ClassVar[Vec3]
    ONE: ClassVar[Vec3]
    NEG_ONE: ClassVar[Vec3]
    MIN: ClassVar[Vec3]
    MAX: ClassVar[Vec3]
    NAN: ClassVar[Vec3]
    INFINITY: ClassVar[Vec3]
    NEG_INFINITY: ClassVar[Vec3]
    X: ClassVar[Vec3]
    Y: ClassVar[Vec3]
    Z: ClassVar[Vec3]
    NEG_X: ClassVar[Vec3]
    NEG_Y: ClassVar[Vec3]
    NEG_Z: ClassVar[Vec3]
    AXES: ClassVar[tuple[Vec3, Vec3, Vec3]]

    def __init__(self, x: float = 0.0, y: float = 0.0, z: float = 0.0) -> None:
        """Initializes a new Vec3 instance.

        Args:
            x: The x-component of the vector.
            y: The y-component of the vector.
            z: The z-component of the vector.
        """

    @staticmethod
    def splat(value: float) -> Vec3:
        """Creates a new Vec3 with all components set to a single value.

        Args:
            value: The float value to apply to x, y, and z.

        Returns:
            A new Vec3 instance (e.g., Vec3(value, value, value)).
        """

    def dot(self, other: Vec3) -> float:
        """Computes the dot product of this vector and another.

        Args:
            other: The other Vec3 to compute the dot product with.

        Returns:
            The dot product as a float.
        """

    def cross(self, other: Vec3) -> Vec3:
        """Computes the cross product of this vector and another.

        Args:
            other: The other Vec3 to compute the cross product with.

        Returns:
            A new Vec3 representing the cross product.
        """

    def length(self) -> float:
        """Calculates the length (magnitude) of the vector.

        Returns:
            The length as a float.
        """

    def normalize(self) -> Vec3:
        """Normalizes the vector to have a length of 1.

        Returns:
            A new Vec3 that is the normalized version of this vector.
        """

    def length_squared(self) -> float: ...
    def __add__(self, other: float | Vec3) -> Vec3: ...
    def __sub__(self, other: float | Vec3) -> Vec3: ...
    def __mul__(self, other: float | Vec3) -> Vec3: ...
    def __truediv__(self, other: float | Vec3) -> Vec3:
        """Divide using IEEE floating-point semantics; zero produces infinities or NaN."""
    def __neg__(self) -> Vec3: ...
    def __rmul__(self, other: float | Vec3) -> Vec3: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def is_normalized(self) -> bool: ...
    def floor(self) -> Vec3: ...
    def ceil(self) -> Vec3: ...
    def zxy(self) -> Vec3: ...
    def zyx(self) -> Vec3: ...
    def min_element(self) -> float: ...
    def max_element(self) -> float: ...
    def xy(self) -> Vec2: ...
    def xz(self) -> Vec2: ...
    def yz(self) -> Vec2: ...
    def yx(self) -> Vec2: ...
    def zx(self) -> Vec2: ...
    def zy(self) -> Vec2: ...
    def xx(self) -> Vec2: ...
    def yy(self) -> Vec2: ...
    def zz(self) -> Vec2: ...
    def xxx(self) -> Vec3: ...
    def xxy(self) -> Vec3: ...
    def xxz(self) -> Vec3: ...
    def xyx(self) -> Vec3: ...
    def xyy(self) -> Vec3: ...
    def xyz(self) -> Vec3: ...
    def xzx(self) -> Vec3: ...
    def xzy(self) -> Vec3: ...
    def xzz(self) -> Vec3: ...
    def yxx(self) -> Vec3: ...
    def yxy(self) -> Vec3: ...
    def yxz(self) -> Vec3: ...
    def yyx(self) -> Vec3: ...
    def yyy(self) -> Vec3: ...
    def yyz(self) -> Vec3: ...
    def yzx(self) -> Vec3: ...
    def yzy(self) -> Vec3: ...
    def yzz(self) -> Vec3: ...
    def zxx(self) -> Vec3: ...
    def zxz(self) -> Vec3: ...
    def zyy(self) -> Vec3: ...
    def zyz(self) -> Vec3: ...
    def zzx(self) -> Vec3: ...
    def zzy(self) -> Vec3: ...
    def zzz(self) -> Vec3: ...
    def with_x(self, x: float) -> Vec3: ...
    def with_y(self, y: float) -> Vec3: ...
    def with_z(self, z: float) -> Vec3: ...
    def truncate(self) -> Vec2: ...
    def extend(self, w: float) -> Vec4: ...
    @staticmethod
    def from_array(a: tuple[float, float, float]) -> Vec3: ...
    def to_array(self) -> tuple[float, float, float]: ...
    def abs(self) -> Vec3: ...
    def signum(self) -> Vec3: ...
    def copysign(self, rhs: Vec3) -> Vec3: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    def abs_diff_eq(self, rhs: Vec3, max_abs_diff: float) -> bool: ...
    def round(self) -> Vec3:
        """Round each component to nearest, with ties away from zero."""
    def trunc(self) -> Vec3: ...
    def fract(self) -> Vec3:
        """Signed fractional part, x - trunc(x)."""
    def fract_gl(self) -> Vec3:
        """GLSL-style fractional part, x - floor(x), in [0, 1)."""
    def exp(self) -> Vec3: ...
    def powf(self, n: float) -> Vec3: ...
    def recip(self) -> Vec3: ...
    def lerp(self, rhs: Vec3, s: float) -> Vec3: ...
    def move_towards(self, rhs: Vec3, d: float) -> Vec3: ...
    def midpoint(self, rhs: Vec3) -> Vec3: ...
    def project_onto(self, rhs: Vec3) -> Vec3: ...
    def reject_from(self, rhs: Vec3) -> Vec3: ...
    def normalize_or_zero(self) -> Vec3: ...
    def try_normalize(self) -> Vec3 | None: ...
    def distance(self, rhs: Vec3) -> float: ...
    def distance_squared(self, rhs: Vec3) -> float: ...
    def min(self, rhs: Vec3) -> Vec3: ...
    def max(self, rhs: Vec3) -> Vec3: ...
    def clamp(self, min: Vec3, max: Vec3) -> Vec3: ...
    def element_sum(self) -> float: ...
    def element_product(self) -> float: ...
    def angle_between(self, other: Vec3) -> float: ...
    def any_orthogonal_vector(self) -> Vec3: ...
    def any_orthonormal_vector(self) -> Vec3:
        """Return one perpendicular unit vector; self must be normalized.

        Raises ValueError for a non-unit or non-finite input.
        """
    def any_orthonormal_pair(self) -> tuple[Vec3, Vec3]:
        """Return two perpendicular unit vectors; self must be normalized.

        Raises ValueError for a non-unit or non-finite input.
        """
    def length_recip(self) -> float:
        """Returns the reciprocal (inverse) of the vector's length."""
    def slerp(self, rhs: Vec3, s: float) -> Vec3:
        """Spherical linear interpolation between two vectors.

        Args:
            rhs: The target vector
            s: Interpolation parameter (0.0 to 1.0)

        Returns:
            Interpolated vector
        """
    def clamp_length(self, min: float, max: float) -> Vec3:
        """Clamps the vector's length to be within the specified range."""
    def clamp_length_min(self, min: float) -> Vec3:
        """Clamps the vector's length to be at least the specified minimum."""
    def clamp_length_max(self, max: float) -> Vec3:
        """Clamps the vector's length to be at most the specified maximum."""
    def project_onto_normalized(self, rhs: Vec3) -> Vec3:
        """Projects this vector onto a normalized vector.

        More efficient than project_onto when rhs is already normalized.
        Raises ValueError if rhs is not unit length or is non-finite.
        """
    def reject_from_normalized(self, rhs: Vec3) -> Vec3:
        """Rejects this vector from a normalized vector.

        More efficient than reject_from when rhs is already normalized.
        Raises ValueError if rhs is not unit length or is non-finite.
        """
    @staticmethod
    def select(mask: tuple[bool, bool, bool], if_true: Vec3, if_false: Vec3) -> Vec3:
        """Component-wise selection based on mask.

        Args:
            mask: Boolean tuple for x, y, z components
            if_true: Vector to use when mask is True
            if_false: Vector to use when mask is False

        Returns:
            Vector with selected components
        """
    def cmpeq(self, rhs: Vec3) -> tuple[bool, bool, bool]:
        """Component-wise equality comparison."""
    def cmpne(self, rhs: Vec3) -> tuple[bool, bool, bool]:
        """Component-wise inequality comparison."""
    def cmpge(self, rhs: Vec3) -> tuple[bool, bool, bool]:
        """Component-wise greater-than-or-equal comparison."""
    def cmpgt(self, rhs: Vec3) -> tuple[bool, bool, bool]:
        """Component-wise greater-than comparison."""
    def cmple(self, rhs: Vec3) -> tuple[bool, bool, bool]:
        """Component-wise less-than-or-equal comparison."""
    def cmplt(self, rhs: Vec3) -> tuple[bool, bool, bool]:
        """Component-wise less-than comparison."""
    @staticmethod
    def from_slice(slice: list[float]) -> Vec3:
        """Creates a Vec3 from a list of floats.

        Args:
            slice: List with at least 3 elements

        Returns:
            Vec3 created from first 3 elements

        Raises:
            ValueError: If slice has fewer than 3 elements
        """
    def __radd__(self, other: float) -> Vec3: ...
    def __rsub__(self, other: float) -> Vec3: ...
    def __rtruediv__(self, other: float) -> Vec3: ...

    def __copy__(self) -> Vec3:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Vec3:
        """Return an independent copy; the value holds no Python objects."""

    def as_uvec3(self) -> UVec3:
        """Cast each component to u32, as glam's `as_uvec3`."""
    def to_vec3a(self) -> Vec3A:
        """Convert to the SIMD-aligned `Vec3A`."""

    def reflect(self, normal: Vec3) -> Vec3:
        """Reflect this vector about `normal`.

        Raises:
            ValueError: If `normal` is not normalized.
        """
    def refract(self, normal: Vec3, eta: float) -> Vec3:
        """Refract this incident direction through a surface with ratio `eta`.

        Raises:
            ValueError: If this vector or `normal` is not normalized.
        """
    def normalize_and_length(self) -> tuple[Vec3, float]:
        """The normalized vector and the original length; `(X, 0.0)` when not normalizable."""
    def rotate_towards(self, rhs: Vec3, max_angle: float) -> Vec3:
        """Rotate towards `rhs` by at most `max_angle` radians."""
    def rotate_x(self, angle: float) -> Vec3: ...
    def rotate_y(self, angle: float) -> Vec3: ...
    def rotate_z(self, angle: float) -> Vec3: ...
    def rotate_axis(self, axis: Vec3, angle: float) -> Vec3:
        """Rotate about `axis` by `angle` radians.

        Raises:
            ValueError: If `axis` is not normalized.
        """

class Vec3A:
    """A 3-dimensional SIMD-aligned vector class.

    Vec3A is similar to Vec3 but uses 16-byte alignment for SIMD operations,
    which can provide performance benefits in compute-intensive scenarios.
    """

    x: float
    y: float
    z: float

    ZERO: ClassVar[Vec3A]
    ONE: ClassVar[Vec3A]
    NEG_ONE: ClassVar[Vec3A]
    MIN: ClassVar[Vec3A]
    MAX: ClassVar[Vec3A]
    NAN: ClassVar[Vec3A]
    INFINITY: ClassVar[Vec3A]
    NEG_INFINITY: ClassVar[Vec3A]
    X: ClassVar[Vec3A]
    Y: ClassVar[Vec3A]
    Z: ClassVar[Vec3A]
    NEG_X: ClassVar[Vec3A]
    NEG_Y: ClassVar[Vec3A]
    NEG_Z: ClassVar[Vec3A]
    AXES: ClassVar[tuple[Vec3A, Vec3A, Vec3A]]

    def __init__(self, x: float = 0.0, y: float = 0.0, z: float = 0.0) -> None: ...
    @staticmethod
    def splat(value: float) -> Vec3A: ...
    @staticmethod
    def from_vec3(v: Vec3) -> Vec3A: ...
    def to_vec3(self) -> Vec3: ...
    def dot(self, other: Vec3A) -> float: ...
    def cross(self, other: Vec3A) -> Vec3A: ...
    def length(self) -> float: ...
    def length_squared(self) -> float: ...
    def normalize(self) -> Vec3A: ...
    def normalize_or_zero(self) -> Vec3A: ...
    def abs(self) -> Vec3A: ...
    def min(self, other: Vec3A) -> Vec3A: ...
    def max(self, other: Vec3A) -> Vec3A: ...
    def distance(self, other: Vec3A) -> float: ...
    def distance_squared(self, other: Vec3A) -> float: ...
    def lerp(self, other: Vec3A, t: float) -> Vec3A: ...
    def __add__(self, other: float | Vec3A) -> Vec3A: ...
    def __sub__(self, other: float | Vec3A) -> Vec3A: ...
    def __mul__(self, other: float | Vec3A) -> Vec3A: ...
    def __rmul__(self, scalar: float) -> Vec3A: ...
    def __truediv__(self, other: float | Vec3A) -> Vec3A:
        """Divide using IEEE floating-point semantics; zero produces infinities or NaN."""
    def __neg__(self) -> Vec3A: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __radd__(self, other: float) -> Vec3A: ...
    def __rsub__(self, other: float) -> Vec3A: ...
    def __rtruediv__(self, other: float) -> Vec3A: ...

    def __copy__(self) -> Vec3A:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Vec3A:
        """Return an independent copy; the value holds no Python objects."""

    def xy(self) -> Vec2: ...
    def xz(self) -> Vec2: ...
    def yz(self) -> Vec2: ...
    def yx(self) -> Vec2: ...
    def zx(self) -> Vec2: ...
    def zy(self) -> Vec2: ...
    def xx(self) -> Vec2: ...
    def yy(self) -> Vec2: ...
    def zz(self) -> Vec2: ...
    def with_x(self, x: float) -> Vec3A: ...
    def with_y(self, y: float) -> Vec3A: ...
    def with_z(self, z: float) -> Vec3A: ...
    def truncate(self) -> Vec2: ...
    def extend(self, w: float) -> Vec4: ...
    @staticmethod
    def from_array(a: tuple[float, float, float]) -> Vec3A: ...
    def to_array(self) -> tuple[float, float, float]: ...
    def signum(self) -> Vec3A: ...
    def copysign(self, rhs: Vec3A) -> Vec3A: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    def abs_diff_eq(self, rhs: Vec3A, max_abs_diff: float) -> bool: ...
    def round(self) -> Vec3A:
        """Round each component to nearest, with ties away from zero."""
    def trunc(self) -> Vec3A: ...
    def fract(self) -> Vec3A:
        """Signed fractional part, x - trunc(x)."""
    def fract_gl(self) -> Vec3A:
        """GLSL-style fractional part, x - floor(x), in [0, 1)."""
    def exp(self) -> Vec3A: ...
    def powf(self, n: float) -> Vec3A: ...
    def recip(self) -> Vec3A: ...
    def move_towards(self, rhs: Vec3A, d: float) -> Vec3A: ...
    def midpoint(self, rhs: Vec3A) -> Vec3A: ...
    def project_onto(self, rhs: Vec3A) -> Vec3A: ...
    def reject_from(self, rhs: Vec3A) -> Vec3A: ...
    def try_normalize(self) -> Vec3A | None: ...
    def clamp(self, min: Vec3A, max: Vec3A) -> Vec3A: ...
    def element_sum(self) -> float: ...
    def element_product(self) -> float: ...
    def is_normalized(self) -> bool: ...
    def floor(self) -> Vec3A: ...
    def ceil(self) -> Vec3A: ...
    def zxy(self) -> Vec3A: ...
    def xyz(self) -> Vec3A: ...
    def xzy(self) -> Vec3A: ...
    def yxz(self) -> Vec3A: ...
    def yzx(self) -> Vec3A: ...
    def zyx(self) -> Vec3A: ...
    def xxx(self) -> Vec3A: ...
    def xxy(self) -> Vec3A: ...
    def xxz(self) -> Vec3A: ...
    def xyx(self) -> Vec3A: ...
    def xyy(self) -> Vec3A: ...
    def xzx(self) -> Vec3A: ...
    def xzz(self) -> Vec3A: ...
    def yxx(self) -> Vec3A: ...
    def yxy(self) -> Vec3A: ...
    def yyx(self) -> Vec3A: ...
    def yyy(self) -> Vec3A: ...
    def yyz(self) -> Vec3A: ...
    def yzy(self) -> Vec3A: ...
    def yzz(self) -> Vec3A: ...
    def zxx(self) -> Vec3A: ...
    def zxz(self) -> Vec3A: ...
    def zyy(self) -> Vec3A: ...
    def zyz(self) -> Vec3A: ...
    def zzx(self) -> Vec3A: ...
    def zzy(self) -> Vec3A: ...
    def zzz(self) -> Vec3A: ...
    def min_element(self) -> float: ...
    def max_element(self) -> float: ...
    def angle_between(self, other: Vec3A) -> float: ...
    def any_orthogonal_vector(self) -> Vec3A: ...
    def any_orthonormal_vector(self) -> Vec3A:
        """Return one perpendicular unit vector; self must be normalized.

        Raises ValueError for a non-unit or non-finite input.
        """
    def any_orthonormal_pair(self) -> tuple[Vec3A, Vec3A]:
        """Return two perpendicular unit vectors; self must be normalized.

        Raises ValueError for a non-unit or non-finite input.
        """
    def length_recip(self) -> float:
        """Returns the reciprocal (inverse) of the vector's length."""
    def slerp(self, rhs: Vec3A, s: float) -> Vec3A:
        """Spherical linear interpolation between two vectors.

        Args:
            rhs: The target vector
            s: Interpolation parameter (0.0 to 1.0)

        Returns:
            Interpolated vector
        """
    def clamp_length(self, min: float, max: float) -> Vec3A:
        """Clamps the vector's length to be within the specified range."""
    def clamp_length_min(self, min: float) -> Vec3A:
        """Clamps the vector's length to be at least the specified minimum."""
    def clamp_length_max(self, max: float) -> Vec3A:
        """Clamps the vector's length to be at most the specified maximum."""
    def project_onto_normalized(self, rhs: Vec3A) -> Vec3A:
        """Projects this vector onto a normalized vector.

        More efficient than project_onto when rhs is already normalized.
        Raises ValueError if rhs is not unit length or is non-finite.
        """
    def reject_from_normalized(self, rhs: Vec3A) -> Vec3A:
        """Rejects this vector from a normalized vector.

        More efficient than reject_from when rhs is already normalized.
        Raises ValueError if rhs is not unit length or is non-finite.
        """
    @staticmethod
    def select(mask: tuple[bool, bool, bool], if_true: Vec3A, if_false: Vec3A) -> Vec3A:
        """Component-wise selection based on mask.

        Args:
            mask: Boolean tuple for x, y, z components
            if_true: Vector to use when mask is True
            if_false: Vector to use when mask is False

        Returns:
            Vector with selected components
        """
    def cmpeq(self, rhs: Vec3A) -> tuple[bool, bool, bool]:
        """Component-wise equality comparison."""
    def cmpne(self, rhs: Vec3A) -> tuple[bool, bool, bool]:
        """Component-wise inequality comparison."""
    def cmpge(self, rhs: Vec3A) -> tuple[bool, bool, bool]:
        """Component-wise greater-than-or-equal comparison."""
    def cmpgt(self, rhs: Vec3A) -> tuple[bool, bool, bool]:
        """Component-wise greater-than comparison."""
    def cmple(self, rhs: Vec3A) -> tuple[bool, bool, bool]:
        """Component-wise less-than-or-equal comparison."""
    def cmplt(self, rhs: Vec3A) -> tuple[bool, bool, bool]:
        """Component-wise less-than comparison."""
    @staticmethod
    def from_slice(slice: list[float]) -> Vec3A:
        """Creates a Vec3A from a list of floats.

        Args:
            slice: List with at least 3 elements

        Returns:
            Vec3A created from first 3 elements

        Raises:
            ValueError: If slice has fewer than 3 elements
        """
    def as_uvec3(self) -> UVec3:
        """Cast each component to u32, as glam's `as_uvec3`."""
    def reflect(self, normal: Vec3A) -> Vec3A:
        """Reflect this vector about `normal`.

        Raises:
            ValueError: If `normal` is not normalized.
        """
    def refract(self, normal: Vec3A, eta: float) -> Vec3A:
        """Refract this incident direction through a surface with ratio `eta`.

        Raises:
            ValueError: If this vector or `normal` is not normalized.
        """
    def normalize_and_length(self) -> tuple[Vec3A, float]:
        """The normalized vector and the original length; `(X, 0.0)` when not normalizable."""
    def rotate_towards(self, rhs: Vec3A, max_angle: float) -> Vec3A:
        """Rotate towards `rhs` by at most `max_angle` radians."""
    def rotate_x(self, angle: float) -> Vec3A: ...
    def rotate_y(self, angle: float) -> Vec3A: ...
    def rotate_z(self, angle: float) -> Vec3A: ...
    def rotate_axis(self, axis: Vec3A, angle: float) -> Vec3A:
        """Rotate about `axis` by `angle` radians.

        Raises:
            ValueError: If `axis` is not normalized.
        """

class Vec2:
    x: float
    y: float
    ZERO: ClassVar[Vec2]
    ONE: ClassVar[Vec2]
    NEG_ONE: ClassVar[Vec2]
    MIN: ClassVar[Vec2]
    MAX: ClassVar[Vec2]
    NAN: ClassVar[Vec2]
    INFINITY: ClassVar[Vec2]
    NEG_INFINITY: ClassVar[Vec2]
    X: ClassVar[Vec2]
    Y: ClassVar[Vec2]
    NEG_X: ClassVar[Vec2]
    NEG_Y: ClassVar[Vec2]
    AXES: ClassVar[tuple[Vec2, Vec2]]

    def __init__(self, x: float = 0.0, y: float = 0.0) -> None: ...
    @staticmethod
    def splat(value: float) -> Vec2: ...
    @staticmethod
    def from_array(a: tuple[float, float]) -> Vec2: ...
    @staticmethod
    def from_slice(slice: list[float]) -> Vec2: ...
    @staticmethod
    def from_angle(angle: float) -> Vec2: ...
    @staticmethod
    def select(mask: tuple[bool, bool], if_true: Vec2, if_false: Vec2) -> Vec2: ...
    def to_array(self) -> tuple[float, float]: ...
    def dot(self, other: Vec2) -> float: ...
    def length(self) -> float: ...
    def length_squared(self) -> float: ...
    def length_recip(self) -> float: ...
    def normalize(self) -> Vec2: ...
    def normalize_or_zero(self) -> Vec2: ...
    def try_normalize(self) -> Vec2 | None: ...
    def is_normalized(self) -> bool: ...
    def distance(self, rhs: Vec2) -> float: ...
    def distance_squared(self, rhs: Vec2) -> float: ...
    def abs(self) -> Vec2: ...
    def signum(self) -> Vec2: ...
    def copysign(self, rhs: Vec2) -> Vec2: ...
    def min(self, rhs: Vec2) -> Vec2: ...
    def max(self, rhs: Vec2) -> Vec2: ...
    def clamp(self, min: Vec2, max: Vec2) -> Vec2: ...
    def clamp_length(self, min: float, max: float) -> Vec2: ...
    def clamp_length_min(self, min: float) -> Vec2: ...
    def clamp_length_max(self, max: float) -> Vec2: ...
    def min_element(self) -> float: ...
    def max_element(self) -> float: ...
    def element_sum(self) -> float: ...
    def element_product(self) -> float: ...
    def floor(self) -> Vec2: ...
    def ceil(self) -> Vec2: ...
    def round(self) -> Vec2:
        """Round each component to nearest, with ties away from zero."""
    def trunc(self) -> Vec2: ...
    def fract(self) -> Vec2:
        """Signed fractional part, x - trunc(x)."""
    def fract_gl(self) -> Vec2:
        """GLSL-style fractional part, x - floor(x), in [0, 1)."""
    def exp(self) -> Vec2: ...
    def powf(self, n: float) -> Vec2: ...
    def recip(self) -> Vec2: ...
    def lerp(self, rhs: Vec2, s: float) -> Vec2: ...
    def move_towards(self, rhs: Vec2, d: float) -> Vec2: ...
    def midpoint(self, rhs: Vec2) -> Vec2: ...
    def project_onto(self, rhs: Vec2) -> Vec2: ...
    def reject_from(self, rhs: Vec2) -> Vec2: ...
    def project_onto_normalized(self, rhs: Vec2) -> Vec2:
        """Project onto a unit rhs; raises ValueError for a non-unit rhs."""
    def reject_from_normalized(self, rhs: Vec2) -> Vec2:
        """Reject from a unit rhs; raises ValueError for a non-unit rhs."""
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    def cmpeq(self, rhs: Vec2) -> tuple[bool, bool]: ...
    def cmpne(self, rhs: Vec2) -> tuple[bool, bool]: ...
    def cmpge(self, rhs: Vec2) -> tuple[bool, bool]: ...
    def cmpgt(self, rhs: Vec2) -> tuple[bool, bool]: ...
    def cmple(self, rhs: Vec2) -> tuple[bool, bool]: ...
    def cmplt(self, rhs: Vec2) -> tuple[bool, bool]: ...
    def angle_to(self, other: Vec2) -> float: ...
    def to_angle(self) -> float: ...
    def xx(self) -> Vec2: ...
    def xy(self) -> Vec2: ...
    def yx(self) -> Vec2: ...
    def yy(self) -> Vec2: ...
    def perp(self) -> Vec2: ...
    def perp_dot(self, other: Vec2) -> float: ...
    def rotate(self, other: Vec2) -> Vec2: ...
    def with_x(self, x: float) -> Vec2: ...
    def with_y(self, y: float) -> Vec2: ...
    def extend(self, z: float) -> Vec3: ...
    def __add__(self, other: float | Vec2) -> Vec2: ...
    def __sub__(self, other: float | Vec2) -> Vec2: ...
    def __mul__(self, other: float | Vec2) -> Vec2: ...
    def __truediv__(self, other: float | Vec2) -> Vec2:
        """Divide using IEEE floating-point semantics; zero produces infinities or NaN."""
    def __neg__(self) -> Vec2: ...
    def __rmul__(self, other: float | Vec2) -> Vec2: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __radd__(self, other: float) -> Vec2: ...
    def __rsub__(self, other: float) -> Vec2: ...
    def __rtruediv__(self, other: float) -> Vec2: ...

    def __copy__(self) -> Vec2:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Vec2:
        """Return an independent copy; the value holds no Python objects."""

    def as_ivec2(self) -> IVec2:
        """Cast each component to i32, as glam's `as_ivec2`."""
    def as_uvec2(self) -> UVec2:
        """Cast each component to u32, as glam's `as_uvec2`."""

    def reflect(self, normal: Vec2) -> Vec2:
        """Reflect this vector about `normal`.

        Raises:
            ValueError: If `normal` is not normalized.
        """
    def refract(self, normal: Vec2, eta: float) -> Vec2:
        """Refract this incident direction through a surface with ratio `eta`.

        Raises:
            ValueError: If this vector or `normal` is not normalized.
        """
    def normalize_and_length(self) -> tuple[Vec2, float]:
        """The normalized vector and the original length; `(X, 0.0)` when not normalizable."""
    def rotate_towards(self, rhs: Vec2, max_angle: float) -> Vec2:
        """Rotate towards `rhs` by at most `max_angle` radians."""
    def abs_diff_eq(self, rhs: Vec2, max_abs_diff: float) -> bool: ...

class Range:
    """A range of float values with start (inclusive) and end (exclusive).

    Equivalent to Rust's Range<f32> (start..end).
    """

    start: float
    end: float

    def __init__(self, start: float, end: float) -> None:
        """Create a Range from start (inclusive) to end (exclusive).

        Args:
            start: The start of the range (inclusive).
            end: The end of the range (exclusive).
        """

    def is_empty(self) -> bool:
        """Returns True if start >= end or either bound is NaN."""

    def contains(self, value: float) -> bool:
        """Returns True if value is within the range [start, end)."""

    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> Range:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Range:
        """Return an independent copy; the value holds no Python objects."""

class Rect:
    """A rectangle defined by minimum and maximum corner points."""

    min: Vec2
    max: Vec2

    def __init__(self, x0: float = 0.0, y0: float = 0.0, x1: float = 0.0, y1: float = 0.0) -> None:
        """Create a Rect from corner coordinates.

        Args:
            x0: X coordinate of the first corner.
            y0: Y coordinate of the first corner.
            x1: X coordinate of the second corner.
            y1: Y coordinate of the second corner.
        """

    @staticmethod
    def from_corners(p0: Vec2, p1: Vec2) -> Rect:
        """Create a Rect from two corner points."""

    @staticmethod
    def from_center_size(origin: Vec2, size: Vec2) -> Rect:
        """Create a Rect from center point and size.

        Raises ValueError if either size component is negative or NaN.
        """

    @staticmethod
    def from_center_half_size(origin: Vec2, half_size: Vec2) -> Rect:
        """Create a Rect from center point and half size.

        Raises ValueError if either half-size component is negative or NaN.
        """

    def center(self) -> Vec2:
        """Get the center point of the rectangle."""

    def size(self) -> Vec2:
        """Get the size of the rectangle."""

    def half_size(self) -> Vec2:
        """Get half the size of the rectangle."""

    def width(self) -> float:
        """Get the width of the rectangle."""

    def height(self) -> float:
        """Get the height of the rectangle."""

    def contains(self, point: Vec2) -> bool:
        """Check if a point is inside the rectangle."""

    def is_empty(self) -> bool:
        """Check if the rectangle has zero area."""

    def intersect(self, other: Rect) -> Rect:
        """Get the intersection of this rectangle with another."""

    def union(self, other: Rect) -> Rect:
        """Get the union of this rectangle with another."""

    def union_point(self, point: Vec2) -> Rect:
        """Expand this rectangle to include a point."""

    def inflate(self, expansion: float) -> Rect:
        """Expand the rectangle by a given amount in all directions."""

    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> Rect:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Rect:
        """Return an independent copy; the value holds no Python objects."""

    def as_irect(self) -> IRect:
        """Cast the corners to i32 coordinates."""
    def as_urect(self) -> URect:
        """Cast the corners to u32 coordinates."""

    EMPTY: ClassVar[Rect]
    def translate(self, offset: Vec2) -> Rect:
        """This rectangle moved by `offset`."""
    def normalize(self, other: Rect) -> Rect:
        """This rectangle in the normalized [0, 1] coordinates of `other`."""

class BVec2:
    """A 2-dimensional boolean vector mask."""

    FALSE: ClassVar[BVec2]
    TRUE: ClassVar[BVec2]

    def __init__(self, x: bool = False, y: bool = False) -> None: ...
    @staticmethod
    def splat(v: bool) -> BVec2: ...
    @staticmethod
    def from_array(a: tuple[bool, bool]) -> BVec2: ...
    @property
    def x(self) -> bool: ...
    @x.setter
    def x(self, value: bool) -> None: ...
    @property
    def y(self) -> bool: ...
    @y.setter
    def y(self, value: bool) -> None: ...
    def bitmask(self) -> int: ...
    def any(self) -> bool: ...
    def all(self) -> bool: ...
    def test(self, index: int) -> bool:
        """Return the element at `index` (0 or 1); raises IndexError otherwise."""
    def set(self, index: int, value: bool) -> None:
        """Set the element at `index` (0 or 1); raises IndexError otherwise."""
    def __and__(self, other: BVec2) -> BVec2: ...
    def __or__(self, other: BVec2) -> BVec2: ...
    def __xor__(self, other: BVec2) -> BVec2: ...
    def __invert__(self) -> BVec2: ...
    def __eq__(self, other: object) -> bool: ...
    def __copy__(self) -> BVec2: ...
    def __deepcopy__(self, memo: dict[int, object]) -> BVec2: ...

class UVec2:
    """A 2-dimensional unsigned integer vector."""

    x: int
    y: int

    ZERO: ClassVar[UVec2]
    ONE: ClassVar[UVec2]
    X: ClassVar[UVec2]
    Y: ClassVar[UVec2]
    MIN: ClassVar[UVec2]
    MAX: ClassVar[UVec2]
    AXES: ClassVar[tuple[UVec2, UVec2]]

    def __init__(self, x: int = 0, y: int = 0) -> None:
        """Create a UVec2 from x and y coordinates.

        Args:
            x: X coordinate (unsigned integer).
            y: Y coordinate (unsigned integer).
        """

    @staticmethod
    def splat(value: int) -> UVec2:
        """Create a UVec2 with all components set to the same value."""

    def min(self, other: UVec2) -> UVec2:
        """Component-wise minimum with another vector."""

    def max(self, other: UVec2) -> UVec2:
        """Component-wise maximum with another vector."""

    def dot(self, other: UVec2) -> int:
        """Compute the dot product with another vector."""

    def length_squared(self) -> int:
        """Get the squared length of the vector."""

    def min_element(self) -> int: ...
    def max_element(self) -> int: ...
    def element_sum(self) -> int: ...
    def element_product(self) -> int: ...
    def with_x(self, x: int) -> UVec2: ...
    def with_y(self, y: int) -> UVec2: ...
    def to_array(self) -> tuple[int, int]: ...

    def __add__(self, other: UVec2) -> UVec2:
        """Componentwise integer arithmetic.

        Raises:
            OverflowError: If any component result is outside its integer range.
        """
    def __sub__(self, other: UVec2) -> UVec2:
        """Componentwise integer arithmetic.

        Raises:
            OverflowError: If any component result is outside its integer range.
        """
    def __mul__(self, other: int | UVec2) -> UVec2:
        """Componentwise integer arithmetic.

        Raises:
            OverflowError: If any component result is outside its integer range.
        """
    def __truediv__(self, other: int | UVec2) -> UVec2:
        """Componentwise integer division.

        Raises:
            ZeroDivisionError: If any divisor component is zero.
            OverflowError: If any result is outside its integer range.
        """
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> UVec2:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> UVec2:
        """Return an independent copy; the value holds no Python objects."""

    def as_vec2(self) -> Vec2:
        """Cast each component to f32."""
    def as_ivec2(self) -> IVec2:
        """Cast each component to i32, as glam's `as_ivec2`."""

    def __rmul__(self, other: int | UVec2) -> UVec2:
        """Componentwise integer multiplication.

        Raises:
            OverflowError: If any component result is outside its integer range.
        """
    def clamp(self, min: UVec2, max: UVec2) -> UVec2:
        """Clamp each component between `min` and `max`.

        Raises:
            ValueError: If any `min` component exceeds the `max` component.
        """
    def cmpeq(self, rhs: UVec2) -> tuple[bool, bool]: ...
    def cmpne(self, rhs: UVec2) -> tuple[bool, bool]: ...
    def cmpge(self, rhs: UVec2) -> tuple[bool, bool]: ...
    def cmpgt(self, rhs: UVec2) -> tuple[bool, bool]: ...
    def cmple(self, rhs: UVec2) -> tuple[bool, bool]: ...
    def cmplt(self, rhs: UVec2) -> tuple[bool, bool]: ...
    @staticmethod
    def from_array(a: tuple[int, int]) -> UVec2: ...
    def manhattan_distance(self, rhs: UVec2) -> int:
        """Sum of the absolute componentwise differences.

        Raises:
            OverflowError: If the distance does not fit in a u32.
        """
    def extend(self, z: int) -> UVec3: ...

class IVec2:
    """A 2-dimensional signed integer vector."""

    x: int
    y: int

    ZERO: ClassVar[IVec2]
    ONE: ClassVar[IVec2]
    NEG_ONE: ClassVar[IVec2]
    X: ClassVar[IVec2]
    Y: ClassVar[IVec2]
    NEG_X: ClassVar[IVec2]
    NEG_Y: ClassVar[IVec2]
    MIN: ClassVar[IVec2]
    MAX: ClassVar[IVec2]
    AXES: ClassVar[tuple[IVec2, IVec2]]

    def __init__(self, x: int = 0, y: int = 0) -> None:
        """Create an IVec2 from x and y coordinates.

        Args:
            x: X coordinate (signed integer).
            y: Y coordinate (signed integer).
        """

    @staticmethod
    def splat(value: int) -> IVec2:
        """Create an IVec2 with all components set to the same value."""

    def min(self, other: IVec2) -> IVec2:
        """Component-wise minimum with another vector."""

    def max(self, other: IVec2) -> IVec2:
        """Component-wise maximum with another vector."""

    def abs(self) -> IVec2:
        """Get absolute value of each component."""

    def signum(self) -> IVec2:
        """Get sign of each component (-1, 0, or 1)."""

    def dot(self, other: IVec2) -> int:
        """Compute the dot product with another vector."""

    def length_squared(self) -> int:
        """Get the squared length of the vector."""

    def __add__(self, other: IVec2) -> IVec2:
        """Componentwise integer arithmetic.

        Raises:
            OverflowError: If any component result is outside its integer range.
        """
    def __sub__(self, other: IVec2) -> IVec2:
        """Componentwise integer arithmetic.

        Raises:
            OverflowError: If any component result is outside its integer range.
        """
    def __mul__(self, other: int | IVec2) -> IVec2:
        """Componentwise integer arithmetic.

        Raises:
            OverflowError: If any component result is outside its integer range.
        """
    def __truediv__(self, other: int | IVec2) -> IVec2:
        """Componentwise integer division.

        Raises:
            ZeroDivisionError: If any divisor component is zero.
            OverflowError: If any result is outside its integer range.
        """
    def __neg__(self) -> IVec2: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> IVec2:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> IVec2:
        """Return an independent copy; the value holds no Python objects."""

    def as_vec2(self) -> Vec2:
        """Cast each component to f32."""
    def as_uvec2(self) -> UVec2:
        """Cast each component to u32, as glam's `as_uvec2`."""

    def __rmul__(self, other: int | IVec2) -> IVec2:
        """Componentwise integer multiplication.

        Raises:
            OverflowError: If any component result is outside its integer range.
        """
    def clamp(self, min: IVec2, max: IVec2) -> IVec2:
        """Clamp each component between `min` and `max`.

        Raises:
            ValueError: If any `min` component exceeds the `max` component.
        """
    def cmpeq(self, rhs: IVec2) -> tuple[bool, bool]: ...
    def cmpne(self, rhs: IVec2) -> tuple[bool, bool]: ...
    def cmpge(self, rhs: IVec2) -> tuple[bool, bool]: ...
    def cmpgt(self, rhs: IVec2) -> tuple[bool, bool]: ...
    def cmple(self, rhs: IVec2) -> tuple[bool, bool]: ...
    def cmplt(self, rhs: IVec2) -> tuple[bool, bool]: ...
    @staticmethod
    def from_array(a: tuple[int, int]) -> IVec2: ...
    def manhattan_distance(self, rhs: IVec2) -> int:
        """Sum of the absolute componentwise differences.

        Raises:
            OverflowError: If the distance does not fit in a u32.
        """
    def element_sum(self) -> int:
        """Sum of the components.

        Raises:
            OverflowError: If the sum is outside the i32 range.
        """
    def element_product(self) -> int:
        """Product of the components.

        Raises:
            OverflowError: If the product is outside the i32 range.
        """
    def to_array(self) -> tuple[int, int]: ...
    def perp(self) -> IVec2:
        """The vector rotated by 90 degrees, `(-y, x)`.

        Raises:
            OverflowError: If `-y` is outside the i32 range.
        """
    def perp_dot(self, rhs: IVec2) -> int:
        """The perpendicular dot product, `x * rhs.y - y * rhs.x`.

        Raises:
            OverflowError: If the result is outside the i32 range.
        """

class UVec3:
    """A 3-dimensional unsigned integer vector."""

    x: int
    y: int
    z: int

    ZERO: ClassVar[UVec3]
    ONE: ClassVar[UVec3]
    X: ClassVar[UVec3]
    Y: ClassVar[UVec3]
    Z: ClassVar[UVec3]
    MIN: ClassVar[UVec3]
    MAX: ClassVar[UVec3]
    AXES: ClassVar[tuple[UVec3, UVec3, UVec3]]

    def __init__(self, x: int = 0, y: int = 0, z: int = 0) -> None:
        """Create a UVec3 from x, y, and z coordinates.

        Args:
            x: X coordinate (unsigned integer).
            y: Y coordinate (unsigned integer).
            z: Z coordinate (unsigned integer).
        """

    @staticmethod
    def splat(value: int) -> UVec3:
        """Create a UVec3 with all components set to the same value."""

    def min(self, other: UVec3) -> UVec3:
        """Component-wise minimum with another vector."""

    def max(self, other: UVec3) -> UVec3:
        """Component-wise maximum with another vector."""

    def dot(self, other: UVec3) -> int:
        """Compute the dot product with another vector."""

    def length_squared(self) -> int:
        """Get the squared length of the vector."""

    def min_element(self) -> int: ...
    def max_element(self) -> int: ...
    def element_sum(self) -> int: ...
    def element_product(self) -> int: ...
    def with_x(self, x: int) -> UVec3: ...
    def with_y(self, y: int) -> UVec3: ...
    def with_z(self, z: int) -> UVec3: ...
    def to_array(self) -> tuple[int, int, int]: ...

    def __add__(self, other: UVec3) -> UVec3:
        """Componentwise integer arithmetic.

        Raises:
            OverflowError: If any component result is outside its integer range.
        """
    def __sub__(self, other: UVec3) -> UVec3:
        """Componentwise integer arithmetic.

        Raises:
            OverflowError: If any component result is outside its integer range.
        """
    def __mul__(self, other: int | UVec3) -> UVec3:
        """Componentwise integer arithmetic.

        Raises:
            OverflowError: If any component result is outside its integer range.
        """
    def __truediv__(self, other: int | UVec3) -> UVec3:
        """Componentwise integer division.

        Raises:
            ZeroDivisionError: If any divisor component is zero.
            OverflowError: If any result is outside its integer range.
        """
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> UVec3:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> UVec3:
        """Return an independent copy; the value holds no Python objects."""

    def as_vec3(self) -> Vec3:
        """Cast each component to f32."""

    def __rmul__(self, other: int | UVec3) -> UVec3:
        """Componentwise integer multiplication.

        Raises:
            OverflowError: If any component result is outside its integer range.
        """
    def clamp(self, min: UVec3, max: UVec3) -> UVec3:
        """Clamp each component between `min` and `max`.

        Raises:
            ValueError: If any `min` component exceeds the `max` component.
        """
    def cmpeq(self, rhs: UVec3) -> tuple[bool, bool, bool]: ...
    def cmpne(self, rhs: UVec3) -> tuple[bool, bool, bool]: ...
    def cmpge(self, rhs: UVec3) -> tuple[bool, bool, bool]: ...
    def cmpgt(self, rhs: UVec3) -> tuple[bool, bool, bool]: ...
    def cmple(self, rhs: UVec3) -> tuple[bool, bool, bool]: ...
    def cmplt(self, rhs: UVec3) -> tuple[bool, bool, bool]: ...
    @staticmethod
    def from_array(a: tuple[int, int, int]) -> UVec3: ...
    def manhattan_distance(self, rhs: UVec3) -> int:
        """Sum of the absolute componentwise differences.

        Raises:
            OverflowError: If the distance does not fit in a u32.
        """

class URect:
    """A rectangle defined by minimum and maximum corner points (unsigned integers).

    The constructors normalise, so `URect(7, 0, 4, 4)` is the same rectangle as
    `URect(4, 0, 7, 4)`. Assigning a corner afterwards does not, matching bevy,
    whose `min` and `max` are plain public fields: pushing `min.x` past `max.x`
    leaves the rectangle inverted, and `width()`, `height()` and `size()` then
    saturate to 0 rather than wrapping to near 2**32.

    `is_empty()` is the check that notices; `contains()` and `center()` do not
    agree with each other on an inverted rectangle.
    """

    min: UVec2
    max: UVec2

    EMPTY: ClassVar[URect]

    def __init__(self, x0: int = 0, y0: int = 0, x1: int = 0, y1: int = 0) -> None:
        """Create a URect from corner coordinates.

        Args:
            x0: X coordinate of the first corner.
            y0: Y coordinate of the first corner.
            x1: X coordinate of the second corner.
            y1: Y coordinate of the second corner.
        """

    @staticmethod
    def from_corners(p0: UVec2, p1: UVec2) -> URect:
        """Create a URect from two corner points."""

    @staticmethod
    def from_center_size(origin: UVec2, size: UVec2) -> URect:
        """Create a URect from center point and size.

        Raises:
            ValueError: If origin is smaller than size / 2 on either axis.
            OverflowError: If the upper corner exceeds u32.
        """

    @staticmethod
    def from_center_half_size(origin: UVec2, half_size: UVec2) -> URect:
        """Create a URect from center point and half size.

        Raises:
            ValueError: If origin is smaller than half_size on either axis.
            OverflowError: If the upper corner exceeds u32.
        """

    def center(self) -> UVec2:
        """Get the center point of the rectangle."""

    def size(self) -> UVec2:
        """Get the size of the rectangle."""

    def half_size(self) -> UVec2:
        """Get half the size of the rectangle."""

    def width(self) -> int:
        """Get the width of the rectangle."""

    def height(self) -> int:
        """Get the height of the rectangle."""

    def inflate(self, expansion: int) -> URect:
        """Expand the rectangle by `expansion` units on every side.

        Negative values shrink it.
        """

    def contains(self, point: UVec2) -> bool:
        """Check if a point is inside the rectangle."""

    def is_empty(self) -> bool:
        """Check if the rectangle has zero area."""

    def intersect(self, other: URect) -> URect:
        """Get the intersection of this rectangle with another."""

    def union(self, other: URect) -> URect:
        """Get the union of this rectangle with another."""

    def union_point(self, point: UVec2) -> URect:
        """Expand this rectangle to include a point."""

    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> URect:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> URect:
        """Return an independent copy; the value holds no Python objects."""

    def as_rect(self) -> Rect:
        """Convert the corners to f32 coordinates."""
    def as_irect(self) -> IRect:
        """Cast the corners to i32 coordinates."""

class IRect:
    """A rectangle defined by minimum and maximum corner points (signed integers)."""

    min: IVec2
    max: IVec2

    EMPTY: ClassVar[IRect]

    def __init__(self, x0: int = 0, y0: int = 0, x1: int = 0, y1: int = 0) -> None:
        """Create an IRect from corner coordinates.

        Args:
            x0: X coordinate of the first corner.
            y0: Y coordinate of the first corner.
            x1: X coordinate of the second corner.
            y1: Y coordinate of the second corner.
        """

    @staticmethod
    def from_corners(p0: IVec2, p1: IVec2) -> IRect:
        """Create an IRect from two corner points."""

    @staticmethod
    def from_center_size(origin: IVec2, size: IVec2) -> IRect:
        """Create an IRect from center point and size."""

    @staticmethod
    def from_center_half_size(origin: IVec2, half_size: IVec2) -> IRect:
        """Create an IRect from center point and half size."""

    def center(self) -> IVec2:
        """Get the center point of the rectangle."""

    def size(self) -> IVec2:
        """Get the size of the rectangle."""

    def half_size(self) -> IVec2:
        """Get half the size of the rectangle."""

    def width(self) -> int:
        """Get the width of the rectangle."""

    def height(self) -> int:
        """Get the height of the rectangle."""

    def inflate(self, expansion: int) -> IRect:
        """Expand the rectangle by `expansion` units on every side.

        Negative values shrink it.
        """

    def contains(self, point: IVec2) -> bool:
        """Check if a point is inside the rectangle."""

    def is_empty(self) -> bool:
        """Check if the rectangle has zero area."""

    def intersect(self, other: IRect) -> IRect:
        """Get the intersection of this rectangle with another."""

    def union(self, other: IRect) -> IRect:
        """Get the union of this rectangle with another."""

    def union_point(self, point: IVec2) -> IRect:
        """Expand this rectangle to include a point."""

    def as_rect(self) -> Rect:
        """Convert this rectangle to a float `Rect`."""

    def as_urect(self) -> URect:
        """Convert this rectangle to a `URect`."""

    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> IRect:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> IRect:
        """Return an independent copy; the value holds no Python objects."""

class EulerRot:
    """Euler rotation order for converting to/from Euler angles."""
    ZYX: EulerRot  # Intrinsic three-axis rotation ZYX
    ZXY: EulerRot  # Intrinsic three-axis rotation ZXY
    YXZ: EulerRot  # Intrinsic three-axis rotation YXZ
    YZX: EulerRot  # Intrinsic three-axis rotation YZX
    XYZ: EulerRot  # Intrinsic three-axis rotation XYZ
    XZY: EulerRot  # Intrinsic three-axis rotation XZY
    ZYZ: EulerRot  # Intrinsic two-axis rotation ZYZ
    ZXZ: EulerRot  # Intrinsic two-axis rotation ZXZ
    YXY: EulerRot  # Intrinsic two-axis rotation YXY
    YZY: EulerRot  # Intrinsic two-axis rotation YZY
    XYX: EulerRot  # Intrinsic two-axis rotation XYX
    XZX: EulerRot  # Intrinsic two-axis rotation XZX
    ZYXEx: EulerRot  # Extrinsic three-axis rotation ZYX
    ZXYEx: EulerRot  # Extrinsic three-axis rotation ZXY
    YXZEx: EulerRot  # Extrinsic three-axis rotation YXZ
    YZXEx: EulerRot  # Extrinsic three-axis rotation YZX
    XYZEx: EulerRot  # Extrinsic three-axis rotation XYZ
    XZYEx: EulerRot  # Extrinsic three-axis rotation XZY
    ZYZEx: EulerRot  # Extrinsic two-axis rotation ZYZ
    ZXZEx: EulerRot  # Extrinsic two-axis rotation ZXZ
    YXYEx: EulerRot  # Extrinsic two-axis rotation YXY
    YZYEx: EulerRot  # Extrinsic two-axis rotation YZY
    XYXEx: EulerRot  # Extrinsic two-axis rotation XYX
    XZXEx: EulerRot  # Extrinsic two-axis rotation XZX

    def __hash__(self) -> int: ...

class Quat:
    IDENTITY: ClassVar[Quat]
    NAN: ClassVar[Quat]

    def __init__(self) -> None:
        """The identity rotation; raw components go through `from_xyzw`."""

    x: float
    y: float
    z: float
    w: float

    @staticmethod
    def from_xyzw(x: float, y: float, z: float, w: float) -> Quat: ...
    @staticmethod
    def from_axis_angle(axis: Vec3, angle: float) -> Quat:
        """Construct from a normalized axis and finite angle.

        Raises:
            ValueError: If the axis is non-finite or not normalized, or the angle is non-finite.
        """
    @staticmethod
    def from_scaled_axis(v: Vec3) -> Quat: ...
    @staticmethod
    def from_rotation_x(x: float) -> Quat: ...
    @staticmethod
    def from_rotation_y(y: float) -> Quat: ...
    @staticmethod
    def from_rotation_z(z: float) -> Quat: ...
    @staticmethod
    def from_rotation_arc(start: Vec3, end: Vec3) -> Quat: ...
    def length(self) -> float: ...
    def length_squared(self) -> float: ...
    def dot(self, rhs: Quat) -> float: ...
    def normalize(self) -> Quat: ...
    def conjugate(self) -> Quat: ...
    def inverse(self) -> Quat: ...
    def lerp(self, rhs: Quat, s: float) -> Quat:
        """Linear interpolation between two quaternions."""
    def slerp(self, rhs: Quat, s: float) -> Quat:
        """Spherical linear interpolation between two quaternions."""
    @staticmethod
    def from_euler(order: EulerRot, x: float, y: float, z: float) -> Quat:
        """Create a quaternion from Euler angles in the given rotation order.

        The angles follow the selected order: ``x`` is its first angle, ``y`` the
        second, and ``z`` the third. For ``EulerRot.XYZ`` and
        ``EulerRot.XYZEx`` the slots coincide with the X, Y, and Z axes
        respectively; for ``EulerRot.ZYX`` and ``EulerRot.ZYXEx`` the first
        slot (parameter ``x``) is the Z-axis angle, and for
        ``EulerRot.YXZ`` and ``EulerRot.YXZEx`` the Y-axis angle.

        Args:
            order: The rotation order (e.g., EulerRot.YXZ)
            x: First angle in radians (first slot of the order)
            y: Second angle in radians (second slot of the order)
            z: Third angle in radians (third slot of the order)

        Returns:
            Quat: The resulting quaternion
        """
    def to_euler(self, order: EulerRot) -> tuple[float, float, float]:
        """Convert quaternion to Euler angles in the given rotation order.

        The returned tuple follows ``order``: entry 0 is its first angle, entry 1 the
        second, entry 2 the third, the same slot order ``from_euler(order,
        ...)`` accepts. For ``EulerRot.ZYX`` entry 0 is the Z-axis angle
        and for ``EulerRot.YXZ`` the Y-axis angle.

        Args:
            order: The rotation order (e.g., EulerRot.YXZ)

        Returns:
            tuple: (first, second, third) angles of the order in radians
        """
    def to_axis_angle(self) -> tuple[Vec3, float]: ...
    def to_scaled_axis(self) -> Vec3: ...
    @overload
    def __mul__(self, other: float) -> Quat: ...
    @overload
    def __mul__(self, other: Quat) -> Quat: ...
    @overload
    def __mul__(self, other: Vec3) -> Vec3: ...
    @overload
    def __mul__(self, other: Vec3A) -> Vec3A: ...
    def __rmul__(self, other: Quat) -> Quat: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __add__(self, other: Quat) -> Quat: ...
    def __sub__(self, other: Quat) -> Quat: ...
    def __truediv__(self, other: float) -> Quat:
        """Divide using IEEE floating-point semantics; zero produces infinities or NaN."""
    def __neg__(self) -> Quat: ...

    def __copy__(self) -> Quat:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Quat:
        """Return an independent copy; the value holds no Python objects."""

    def mul_vec3(self, rhs: Vec3) -> Vec3:
        """Rotate `rhs` by this quaternion.

        Raises:
            ValueError: If a quaternion is not normalized.
        """
    def mul_quat(self, rhs: Quat) -> Quat: ...
    def is_normalized(self) -> bool: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    def is_near_identity(self) -> bool: ...
    def angle_between(self, rhs: Quat) -> float:
        """The angle in radians between two rotations.

        Raises:
            ValueError: If a quaternion is not normalized.
        """
    def rotate_towards(self, rhs: Quat, max_angle: float) -> Quat:
        """Rotate towards `rhs` by at most `max_angle` radians.

        Raises:
            ValueError: If a quaternion is not normalized.
        """
    @staticmethod
    def from_mat3(mat: Mat3) -> Quat:
        """The rotation of a pure-rotation matrix.

        Raises:
            ValueError: If the rotation axes are not normalized.
        """
    @staticmethod
    def from_mat4(mat: Mat4) -> Quat:
        """The rotation of a matrix's upper 3x3 block.

        Raises:
            ValueError: If the rotation axes are not normalized.
        """
    @staticmethod
    def from_affine3a(a: Affine3A) -> Quat:
        """The rotation of an affine transform's matrix.

        Raises:
            ValueError: If the rotation axes are not normalized.
        """
    @staticmethod
    def look_to_lh(dir: Vec3, up: Vec3) -> Quat:
        """Left-handed view rotation facing `dir`.

        Raises:
            ValueError: If `dir` or `up` is not normalized or they are parallel.
        """
    @staticmethod
    def look_to_rh(dir: Vec3, up: Vec3) -> Quat:
        """Right-handed view rotation facing `dir`.

        Raises:
            ValueError: If `dir` or `up` is not normalized or they are parallel.
        """
    @staticmethod
    def look_at_lh(eye: Vec3, center: Vec3, up: Vec3) -> Quat:
        """Left-handed view rotation from `eye` toward `center`.

        Raises:
            ValueError: If `eye` equals `center` or the view direction is parallel to `up`.
        """
    @staticmethod
    def look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Quat:
        """Right-handed view rotation from `eye` toward `center`.

        Raises:
            ValueError: If `eye` equals `center` or the view direction is parallel to `up`.
        """
    @staticmethod
    def from_rotation_arc_2d(from_: Vec2, to: Vec2) -> Quat:
        """The rotation about Z from `from_` to `to`.

        Raises:
            ValueError: If `from_` or `to` is not normalized.
        """
    @staticmethod
    def from_rotation_arc_colinear(from_: Vec3, to: Vec3) -> Quat:
        """The smallest rotation aligning `from_` with `to` or `-to`.

        Raises:
            ValueError: If `from_` or `to` is not normalized.
        """
    @staticmethod
    def from_array(a: tuple[float, float, float, float]) -> Quat: ...
    def to_array(self) -> tuple[float, float, float, float]: ...
    def abs_diff_eq(self, rhs: Quat, max_abs_diff: float) -> bool: ...

class Dir3:
    X: Dir3
    Y: Dir3
    Z: Dir3
    NEG_X: Dir3
    NEG_Y: Dir3
    NEG_Z: Dir3

    def __init__(self, x: float, y: float, z: float) -> None: ...
    @staticmethod
    def from_vec3(vec: Vec3) -> Dir3: ...
    @property
    def x(self) -> float: ...
    @property
    def y(self) -> float: ...
    @property
    def z(self) -> float: ...
    def as_vec3(self) -> Vec3: ...
    def dot(self, other: Dir3) -> float: ...
    def cross(self, other: Vec3) -> Vec3:
        """Return the native cross product, including nonunit and zero operands."""
    def slerp(self, rhs: Dir3, s: float) -> Dir3: ...
    def fast_renormalize(self) -> Dir3:
        """Quickly renormalize using a first-order Taylor approximation."""
    @staticmethod
    def from_xyz_unchecked(x: float, y: float, z: float) -> Dir3:
        """Create a direction without checking if it's normalized."""
    @staticmethod
    def new_unchecked(value: Vec3) -> Dir3:
        """Create a direction from a Vec3 without checking normalization."""
    def interpolate_stable(self, other: Dir3, t: float) -> Dir3:
        """Stable interpolation between two directions."""
    def interpolate_stable_assign(self, other: Dir3, t: float) -> None:
        """Stable interpolation, assigning result to self."""
    def smooth_nudge(self, target: Dir3, decay_rate: float, delta: float) -> None:
        """Smoothly nudge towards target at given decay rate."""
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __neg__(self) -> Dir3: ...
    def __mul__(self, other: float) -> Vec3: ...
    @overload
    def __rmul__(self, other: float) -> Vec3: ...
    @overload
    def __rmul__(self, other: Quat) -> Dir3: ...

    def __copy__(self) -> Dir3:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Dir3:
        """Return an independent copy; the value holds no Python objects."""

    @staticmethod
    def from_xyz(x: float, y: float, z: float) -> Dir3:
        """Create a direction from x, y, and z components.

        Raises:
            ValueError: If the vector length is zero, very close to zero, or non-finite
        """
    @staticmethod
    def new_and_length(value: Vec3) -> tuple[Dir3, float]:
        """Normalize `value`, returning the direction and the original length.

        Raises:
            ValueError: If the vector length is zero, very close to zero, or non-finite
        """

class Mat2:
    """2x2 column-major matrix for rotation/scale/shear."""

    IDENTITY: ClassVar[Mat2]
    ZERO: ClassVar[Mat2]
    NAN: ClassVar[Mat2]
    def __init__(self, *, x_axis: Vec2 = Vec2.X, y_axis: Vec2 = Vec2.Y) -> None: ...
    @staticmethod
    def from_cols(x_axis: Vec2, y_axis: Vec2) -> Mat2: ...
    @staticmethod
    def from_cols_array(values: list[float]) -> Mat2: ...
    @staticmethod
    def from_angle(angle: float) -> Mat2: ...
    @staticmethod
    def from_scale_angle(scale: Vec2, angle: float) -> Mat2: ...
    @staticmethod
    def from_diagonal(diagonal: Vec2) -> Mat2: ...
    def col(self, index: int) -> Vec2:
        """The matrix's column at `index`, counting from the end if negative.

        Raises:
            IndexError: If the index is outside -2 to 1
        """
    def row(self, index: int) -> Vec2:
        """The matrix's row at `index`, counting from the end if negative.

        Raises:
            IndexError: If the index is outside -2 to 1
        """
    @property
    def x_axis(self) -> Vec2: ...
    @x_axis.setter
    def x_axis(self, value: Vec2) -> None: ...
    @property
    def y_axis(self) -> Vec2: ...
    @y_axis.setter
    def y_axis(self, value: Vec2) -> None: ...
    def transpose(self) -> Mat2: ...
    def determinant(self) -> float: ...
    def inverse(self) -> Mat2:
        """Return the inverse matrix.

        Raises:
            ValueError: If the matrix is singular or its inverse is not finite
        """
    def mul_vec2(self, rhs: Vec2) -> Vec2: ...
    def mul_mat2(self, rhs: Mat2) -> Mat2: ...
    def mul_scalar(self, rhs: float) -> Mat2: ...
    def abs(self) -> Mat2: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    @overload
    def __mul__(self, other: Mat2) -> Mat2: ...
    @overload
    def __mul__(self, other: float) -> Mat2: ...
    @overload
    def __mul__(self, other: Vec2) -> Vec2: ...
    def __add__(self, other: Mat2) -> Mat2: ...
    def __sub__(self, other: Mat2) -> Mat2: ...
    def __neg__(self) -> Mat2: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __rmul__(self, other: float) -> Mat2: ...

    def __copy__(self) -> Mat2:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Mat2:
        """Return an independent copy; the value holds no Python objects."""

    def to_cols_array(self) -> list[float]: ...
    @staticmethod
    def from_cols_array_2d(m: list[list[float]]) -> Mat2: ...
    def to_cols_array_2d(self) -> list[list[float]]: ...
    def add_mat2(self, rhs: Mat2) -> Mat2: ...
    def sub_mat2(self, rhs: Mat2) -> Mat2: ...
    def div_scalar(self, rhs: float) -> Mat2: ...

class Affine2:
    """2D affine transformation (scale, shear, rotation, translation)."""

    IDENTITY: ClassVar[Affine2]
    ZERO: ClassVar[Affine2]
    NAN: ClassVar[Affine2]
    def __init__(
        self, *, matrix2: Mat2 = Mat2.IDENTITY, translation: Vec2 = Vec2.ZERO
    ) -> None: ...
    @staticmethod
    def from_cols(x_axis: Vec2, y_axis: Vec2, z_axis: Vec2) -> Affine2: ...
    @staticmethod
    def from_scale(scale: Vec2) -> Affine2: ...
    @staticmethod
    def from_angle(angle: float) -> Affine2: ...
    @staticmethod
    def from_translation(translation: Vec2) -> Affine2: ...
    @staticmethod
    def from_scale_angle_translation(
        scale: Vec2, angle: float, translation: Vec2
    ) -> Affine2: ...
    @staticmethod
    def from_mat2(matrix2: Mat2) -> Affine2: ...
    @staticmethod
    def from_mat2_translation(matrix2: Mat2, translation: Vec2) -> Affine2: ...
    def to_scale_angle_translation(self) -> tuple[Vec2, float, Vec2]: ...
    @property
    def matrix2(self) -> Mat2: ...
    @matrix2.setter
    def matrix2(self, value: Mat2) -> None: ...
    @property
    def translation(self) -> Vec2: ...
    @translation.setter
    def translation(self, value: Vec2) -> None: ...
    def inverse(self) -> Affine2:
        """Return the inverse matrix.

        Raises:
            ValueError: If the matrix is singular or its inverse is not finite
        """
    def transform_point2(self, point: Vec2) -> Vec2: ...
    def transform_vector2(self, vector: Vec2) -> Vec2: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    def into_mat3(self) -> Mat3: ...
    def __mul__(self, other: Affine2) -> Affine2: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> Affine2:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Affine2:
        """Return an independent copy; the value holds no Python objects."""

class Mat3A:
    """A 3x3 column-major matrix using SIMD-aligned Vec3A columns.

    Provides optimized 3x3 matrix operations with SIMD alignment.
    """

    IDENTITY: ClassVar[Mat3A]
    ZERO: ClassVar[Mat3A]
    NAN: ClassVar[Mat3A]

    def __init__(
        self, *, x_axis: Vec3A = Vec3A.X, y_axis: Vec3A = Vec3A.Y, z_axis: Vec3A = Vec3A.Z
    ) -> None: ...

    @staticmethod
    def from_cols(x_axis: Vec3A, y_axis: Vec3A, z_axis: Vec3A) -> Mat3A: ...
    @staticmethod
    def from_cols_array(m: list[float]) -> Mat3A: ...
    @staticmethod
    def from_diagonal(diagonal: Vec3) -> Mat3A: ...
    @staticmethod
    def from_rotation_x(angle: float) -> Mat3A: ...
    @staticmethod
    def from_rotation_y(angle: float) -> Mat3A: ...
    @staticmethod
    def from_rotation_z(angle: float) -> Mat3A: ...

    @property
    def x_axis(self) -> Vec3A: ...
    @x_axis.setter
    def x_axis(self, value: Vec3A) -> None: ...
    @property
    def y_axis(self) -> Vec3A: ...
    @y_axis.setter
    def y_axis(self, value: Vec3A) -> None: ...
    @property
    def z_axis(self) -> Vec3A: ...
    @z_axis.setter
    def z_axis(self, value: Vec3A) -> None: ...

    def col(self, index: int) -> Vec3A:
        """The matrix's column at `index`, counting from the end if negative.

        Raises:
            IndexError: If the index is outside -3 to 2
        """

    def row(self, index: int) -> Vec3A:
        """The matrix's row at `index`, counting from the end if negative.

        Raises:
            IndexError: If the index is outside -3 to 2
        """
    def transpose(self) -> Mat3A: ...
    def determinant(self) -> float: ...
    def inverse(self) -> Mat3A:
        """Return the inverse matrix.

        Raises:
            ValueError: If the matrix is singular or its inverse is not finite
        """
    def mul_vec3a(self, rhs: Vec3A) -> Vec3A: ...
    def mul_mat3(self, rhs: Mat3A) -> Mat3A: ...
    def mul_scalar(self, rhs: float) -> Mat3A: ...
    def to_cols_array(self) -> list[float]: ...
    def abs(self) -> Mat3A: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...

    @overload
    def __mul__(self, other: float) -> Mat3A: ...
    @overload
    def __mul__(self, other: Mat3A) -> Mat3A: ...
    @overload
    def __mul__(self, other: Vec3A) -> Vec3A: ...
    @overload
    def __mul__(self, other: Vec3) -> Vec3: ...
    def __rmul__(self, scalar: float) -> Mat3A: ...
    def __neg__(self) -> Mat3A: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __add__(self, other: Mat3A) -> Mat3A: ...
    def __sub__(self, other: Mat3A) -> Mat3A: ...
    def __truediv__(self, other: float) -> Mat3A:
        """Divide using IEEE floating-point semantics; zero produces infinities or NaN."""
    def __rtruediv__(self, other: float) -> Mat3A: ...

    def __copy__(self) -> Mat3A:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Mat3A:
        """Return an independent copy; the value holds no Python objects."""

    @staticmethod
    def from_cols_array_2d(m: list[list[float]]) -> Mat3A: ...
    def to_cols_array_2d(self) -> list[list[float]]: ...
    def add_mat3(self, rhs: Mat3A) -> Mat3A: ...
    def sub_mat3(self, rhs: Mat3A) -> Mat3A: ...
    def div_scalar(self, rhs: float) -> Mat3A: ...
    def mul_vec3(self, rhs: Vec3) -> Vec3: ...

class Affine3A:
    """A 3D affine transform using SIMD-aligned types.

    Composed of a 3x3 matrix (rotation/scale/shear) and a translation vector.
    """

    IDENTITY: ClassVar[Affine3A]
    ZERO: ClassVar[Affine3A]
    NAN: ClassVar[Affine3A]

    def __init__(
        self, *, matrix3: Mat3A = Mat3A.IDENTITY, translation: Vec3A = Vec3A.ZERO
    ) -> None: ...

    @staticmethod
    def from_cols(x_axis: Vec3A, y_axis: Vec3A, z_axis: Vec3A, w_axis: Vec3A) -> Affine3A: ...
    @staticmethod
    def from_translation(translation: Vec3) -> Affine3A: ...
    @staticmethod
    def from_quat(rotation: Quat) -> Affine3A: ...
    @staticmethod
    def from_axis_angle(axis: Vec3, angle: float) -> Affine3A: ...
    @staticmethod
    def from_rotation_x(angle: float) -> Affine3A: ...
    @staticmethod
    def from_rotation_y(angle: float) -> Affine3A: ...
    @staticmethod
    def from_rotation_z(angle: float) -> Affine3A: ...
    @staticmethod
    def from_scale(scale: Vec3) -> Affine3A: ...
    @staticmethod
    def from_scale_rotation_translation(
        scale: Vec3, rotation: Quat, translation: Vec3
    ) -> Affine3A: ...
    @staticmethod
    def from_rotation_translation(rotation: Quat, translation: Vec3) -> Affine3A: ...
    @staticmethod
    def from_mat4(mat: Mat4) -> Affine3A: ...

    @property
    def matrix3(self) -> Mat3A: ...
    @matrix3.setter
    def matrix3(self, value: Mat3A) -> None: ...
    @property
    def translation(self) -> Vec3A: ...
    @translation.setter
    def translation(self, value: Vec3A) -> None: ...

    def transform_point3(self, rhs: Vec3) -> Vec3: ...
    def transform_point3a(self, rhs: Vec3A) -> Vec3A: ...
    def transform_vector3(self, rhs: Vec3) -> Vec3: ...
    def transform_vector3a(self, rhs: Vec3A) -> Vec3A: ...
    def inverse(self) -> Affine3A:
        """Return the inverse matrix.

        Raises:
            ValueError: If the matrix is singular or its inverse is not finite
        """
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...

    def __mul__(self, other: Affine3A) -> Affine3A: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> Affine3A:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Affine3A:
        """Return an independent copy; the value holds no Python objects."""

    def to_scale_rotation_translation(self) -> tuple[Vec3, Quat, Vec3]:
        """Decompose into scale, rotation, and translation.

        Raises:
            ValueError: If the determinant is zero or not finite.
        """
    @staticmethod
    def look_to_lh(eye: Vec3, dir: Vec3, up: Vec3) -> Affine3A:
        """Left-handed view transform at `eye` facing `dir`.

        Raises:
            ValueError: If `dir` or `up` is not normalized or they are parallel.
        """
    @staticmethod
    def look_to_rh(eye: Vec3, dir: Vec3, up: Vec3) -> Affine3A:
        """Right-handed view transform at `eye` facing `dir`.

        Raises:
            ValueError: If `dir` or `up` is not normalized or they are parallel.
        """
    @staticmethod
    def look_at_lh(eye: Vec3, center: Vec3, up: Vec3) -> Affine3A:
        """Left-handed view transform from `eye` toward `center`.

        Raises:
            ValueError: If `eye` equals `center` or the view direction is parallel to `up`.
        """
    @staticmethod
    def look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Affine3A:
        """Right-handed view transform from `eye` toward `center`.

        Raises:
            ValueError: If `eye` equals `center` or the view direction is parallel to `up`.
        """

class Vec4:
    """A 4-dimensional vector class."""
    x: float
    y: float
    z: float
    w: float

    ZERO: ClassVar[Vec4]
    ONE: ClassVar[Vec4]
    NEG_ONE: ClassVar[Vec4]
    MIN: ClassVar[Vec4]
    MAX: ClassVar[Vec4]
    NAN: ClassVar[Vec4]
    INFINITY: ClassVar[Vec4]
    NEG_INFINITY: ClassVar[Vec4]
    X: ClassVar[Vec4]
    Y: ClassVar[Vec4]
    Z: ClassVar[Vec4]
    W: ClassVar[Vec4]
    NEG_X: ClassVar[Vec4]
    NEG_Y: ClassVar[Vec4]
    NEG_Z: ClassVar[Vec4]
    NEG_W: ClassVar[Vec4]
    AXES: ClassVar[tuple[Vec4, Vec4, Vec4, Vec4]]

    def __init__(self, x: float = 0.0, y: float = 0.0, z: float = 0.0, w: float = 0.0) -> None: ...
    @staticmethod
    def splat(value: float) -> Vec4: ...
    def dot(self, other: Vec4) -> float: ...
    def length(self) -> float: ...
    def length_squared(self) -> float: ...
    def length_recip(self) -> float: ...
    def normalize(self) -> Vec4: ...
    def is_normalized(self) -> bool: ...
    def floor(self) -> Vec4: ...
    def ceil(self) -> Vec4: ...
    def min_element(self) -> float: ...
    def max_element(self) -> float: ...
    def truncate(self) -> Vec3: ...
    def xyz(self) -> Vec3: ...
    def xyzw(self) -> Vec4: ...
    @staticmethod
    def from_array(a: tuple[float, float, float, float]) -> Vec4: ...
    @staticmethod
    def from_slice(a: list[float]) -> Vec4: ...
    @staticmethod
    def select(
        mask: tuple[bool, bool, bool, bool], if_true: Vec4, if_false: Vec4
    ) -> Vec4: ...
    def to_array(self) -> tuple[float, float, float, float]: ...
    def abs(self) -> Vec4: ...
    def signum(self) -> Vec4: ...
    def copysign(self, rhs: Vec4) -> Vec4: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    def round(self) -> Vec4:
        """Round each component to nearest, with ties away from zero."""
    def trunc(self) -> Vec4: ...
    def fract(self) -> Vec4:
        """Signed fractional part, x - trunc(x)."""
    def fract_gl(self) -> Vec4:
        """GLSL-style fractional part, x - floor(x), in [0, 1)."""
    def exp(self) -> Vec4: ...
    def powf(self, n: float) -> Vec4: ...
    def recip(self) -> Vec4: ...
    def lerp(self, rhs: Vec4, s: float) -> Vec4: ...
    def move_towards(self, rhs: Vec4, d: float) -> Vec4: ...
    def midpoint(self, rhs: Vec4) -> Vec4: ...
    def project_onto(self, rhs: Vec4) -> Vec4: ...
    def reject_from(self, rhs: Vec4) -> Vec4: ...
    def project_onto_normalized(self, rhs: Vec4) -> Vec4:
        """Project onto a unit rhs; raises ValueError for a non-unit rhs."""
    def reject_from_normalized(self, rhs: Vec4) -> Vec4:
        """Reject from a unit rhs; raises ValueError for a non-unit rhs."""
    def normalize_or_zero(self) -> Vec4: ...
    def try_normalize(self) -> Vec4 | None: ...
    def distance(self, rhs: Vec4) -> float: ...
    def distance_squared(self, rhs: Vec4) -> float: ...
    def min(self, rhs: Vec4) -> Vec4: ...
    def max(self, rhs: Vec4) -> Vec4: ...
    def clamp(self, min: Vec4, max: Vec4) -> Vec4: ...
    def clamp_length(self, min: float, max: float) -> Vec4: ...
    def clamp_length_min(self, min: float) -> Vec4: ...
    def clamp_length_max(self, max: float) -> Vec4: ...
    def with_x(self, x: float) -> Vec4: ...
    def with_y(self, y: float) -> Vec4: ...
    def with_z(self, z: float) -> Vec4: ...
    def with_w(self, w: float) -> Vec4: ...
    def cmpeq(self, rhs: Vec4) -> tuple[bool, bool, bool, bool]: ...
    def cmpne(self, rhs: Vec4) -> tuple[bool, bool, bool, bool]: ...
    def cmplt(self, rhs: Vec4) -> tuple[bool, bool, bool, bool]: ...
    def cmple(self, rhs: Vec4) -> tuple[bool, bool, bool, bool]: ...
    def cmpgt(self, rhs: Vec4) -> tuple[bool, bool, bool, bool]: ...
    def cmpge(self, rhs: Vec4) -> tuple[bool, bool, bool, bool]: ...
    def xx(self) -> Vec2: ...
    def xy(self) -> Vec2: ...
    def xz(self) -> Vec2: ...
    def xw(self) -> Vec2: ...
    def yx(self) -> Vec2: ...
    def yy(self) -> Vec2: ...
    def yz(self) -> Vec2: ...
    def yw(self) -> Vec2: ...
    def zx(self) -> Vec2: ...
    def zy(self) -> Vec2: ...
    def zz(self) -> Vec2: ...
    def zw(self) -> Vec2: ...
    def wx(self) -> Vec2: ...
    def wy(self) -> Vec2: ...
    def wz(self) -> Vec2: ...
    def ww(self) -> Vec2: ...
    def element_sum(self) -> float: ...
    def element_product(self) -> float: ...
    def __add__(self, other: float | Vec4) -> Vec4: ...
    def __sub__(self, other: float | Vec4) -> Vec4: ...
    def __mul__(self, other: float | Vec4) -> Vec4: ...
    def __truediv__(self, other: float | Vec4) -> Vec4:
        """Divide using IEEE floating-point semantics; zero produces infinities or NaN."""
    def __neg__(self) -> Vec4: ...
    def __rmul__(self, other: float | Vec4) -> Vec4: ...
    def __radd__(self, other: float) -> Vec4: ...
    def __rsub__(self, other: float) -> Vec4: ...
    def __rtruediv__(self, other: float) -> Vec4: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> Vec4:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Vec4:
        """Return an independent copy; the value holds no Python objects."""

    def reflect(self, normal: Vec4) -> Vec4:
        """Reflect this vector about `normal`.

        Raises:
            ValueError: If `normal` is not normalized.
        """
    def refract(self, normal: Vec4, eta: float) -> Vec4:
        """Refract this incident direction through a surface with ratio `eta`.

        Raises:
            ValueError: If this vector or `normal` is not normalized.
        """
    def normalize_and_length(self) -> tuple[Vec4, float]:
        """The normalized vector and the original length; `(X, 0.0)` when not normalizable."""
    def abs_diff_eq(self, rhs: Vec4, max_abs_diff: float) -> bool: ...

class Mat3:
    """A 3x3 matrix class."""
    IDENTITY: ClassVar[Mat3]
    ZERO: ClassVar[Mat3]
    NAN: ClassVar[Mat3]

    def __init__(
        self, *, x_axis: Vec3 = Vec3.X, y_axis: Vec3 = Vec3.Y, z_axis: Vec3 = Vec3.Z
    ) -> None: ...

    @staticmethod
    def from_cols(x_axis: Vec3, y_axis: Vec3, z_axis: Vec3) -> Mat3: ...
    @staticmethod
    def from_cols_array(m: list[float]) -> Mat3: ...
    @staticmethod
    def from_cols_array_2d(m: list[list[float]]) -> Mat3: ...
    @staticmethod
    def from_diagonal(diagonal: Vec3) -> Mat3: ...
    @staticmethod
    def from_quat(quat: Quat) -> Mat3: ...
    @staticmethod
    def from_axis_angle(axis: Vec3, angle: float) -> Mat3: ...
    @staticmethod
    def from_rotation_x(angle: float) -> Mat3: ...
    @staticmethod
    def from_rotation_y(angle: float) -> Mat3: ...
    @staticmethod
    def from_rotation_z(angle: float) -> Mat3: ...
    @staticmethod
    def from_translation(translation: Vec2) -> Mat3: ...
    @staticmethod
    def from_angle(angle: float) -> Mat3: ...
    @staticmethod
    def from_scale(scale: Vec2) -> Mat3: ...
    @staticmethod
    def from_scale_angle_translation(scale: Vec2, angle: float, translation: Vec2) -> Mat3: ...

    def col(self, index: int) -> Vec3:
        """The matrix's column at `index`, counting from the end if negative.

        Raises:
            IndexError: If the index is outside -3 to 2
        """
    def row(self, index: int) -> Vec3:
        """The matrix's row at `index`, counting from the end if negative.

        Raises:
            IndexError: If the index is outside -3 to 2
        """
    def to_cols_array(self) -> list[float]: ...
    def to_cols_array_2d(self) -> list[list[float]]: ...
    def transpose(self) -> Mat3: ...
    def determinant(self) -> float: ...
    def inverse(self) -> Mat3:
        """Return the inverse matrix.

        Raises:
            ValueError: If the matrix is singular or its inverse is not finite
        """
    def mul_vec3(self, rhs: Vec3) -> Vec3: ...
    def mul_mat3(self, rhs: Mat3) -> Mat3: ...
    def add_mat3(self, rhs: Mat3) -> Mat3: ...
    def sub_mat3(self, rhs: Mat3) -> Mat3: ...
    def mul_scalar(self, rhs: float) -> Mat3: ...
    def div_scalar(self, rhs: float) -> Mat3: ...
    def transform_point2(self, rhs: Vec2) -> Vec2: ...
    def transform_vector2(self, rhs: Vec2) -> Vec2: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    def abs(self) -> Mat3: ...
    def abs_diff_eq(self, rhs: Mat3, max_abs_diff: float) -> bool: ...

    def __add__(self, other: Mat3) -> Mat3: ...
    def __sub__(self, other: Mat3) -> Mat3: ...
    @overload
    def __mul__(self, other: float) -> Mat3: ...
    @overload
    def __mul__(self, other: Mat3) -> Mat3: ...
    @overload
    def __mul__(self, other: Vec3) -> Vec3: ...
    def __rmul__(self, other: float) -> Mat3: ...
    def __truediv__(self, scalar: float) -> Mat3: ...
    def __neg__(self) -> Mat3: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> Mat3:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Mat3:
        """Return an independent copy; the value holds no Python objects."""

    @property
    def x_axis(self) -> Vec3:
        """The first column; nested writes reach this matrix."""
    @x_axis.setter
    def x_axis(self, value: Vec3) -> None: ...
    @property
    def y_axis(self) -> Vec3:
        """The second column; nested writes reach this matrix."""
    @y_axis.setter
    def y_axis(self, value: Vec3) -> None: ...
    @property
    def z_axis(self) -> Vec3:
        """The third column; nested writes reach this matrix."""
    @z_axis.setter
    def z_axis(self, value: Vec3) -> None: ...
    @staticmethod
    def from_euler(order: EulerRot, a: float, b: float, c: float) -> Mat3: ...
    def to_euler(self, order: EulerRot) -> tuple[float, float, float]:
        """Extract Euler angles in the given order.

        Raises:
            ValueError: If a column is not normalized.
        """
    @staticmethod
    def from_mat4(m: Mat4) -> Mat3:
        """The upper-left 3x3 block of a Mat4."""
    @staticmethod
    def look_to_lh(dir: Vec3, up: Vec3) -> Mat3:
        """Left-handed view rotation facing `dir`.

        Raises:
            ValueError: If `dir` or `up` is not normalized or they are parallel.
        """
    @staticmethod
    def look_to_rh(dir: Vec3, up: Vec3) -> Mat3:
        """Right-handed view rotation facing `dir`.

        Raises:
            ValueError: If `dir` or `up` is not normalized or they are parallel.
        """
    @staticmethod
    def look_at_lh(eye: Vec3, center: Vec3, up: Vec3) -> Mat3:
        """Left-handed view rotation from `eye` toward `center`.

        Raises:
            ValueError: If `eye` equals `center` or the view direction is parallel to `up`.
        """
    @staticmethod
    def look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Mat3:
        """Right-handed view rotation from `eye` toward `center`.

        Raises:
            ValueError: If `eye` equals `center` or the view direction is parallel to `up`.
        """

class Mat4:
    """A 4x4 matrix class for 3D transformations."""

    IDENTITY: ClassVar[Mat4]
    ZERO: ClassVar[Mat4]
    NAN: ClassVar[Mat4]

    def __init__(
        self,
        *,
        x_axis: Vec4 = Vec4.X,
        y_axis: Vec4 = Vec4.Y,
        z_axis: Vec4 = Vec4.Z,
        w_axis: Vec4 = Vec4.W,
    ) -> None: ...

    @staticmethod
    def from_cols(x_axis: Vec4, y_axis: Vec4, z_axis: Vec4, w_axis: Vec4) -> Mat4: ...
    @staticmethod
    def from_cols_array(m: list[float]) -> Mat4: ...
    @staticmethod
    def from_cols_array_2d(m: list[list[float]]) -> Mat4: ...
    @staticmethod
    def from_diagonal(diagonal: Vec4) -> Mat4: ...
    @staticmethod
    def from_translation(translation: Vec3) -> Mat4: ...
    @staticmethod
    def from_scale(scale: Vec3) -> Mat4: ...
    @staticmethod
    def from_quat(quat: Quat) -> Mat4: ...
    @staticmethod
    def from_axis_angle(axis: Vec3, angle: float) -> Mat4: ...
    @staticmethod
    def from_rotation_x(angle: float) -> Mat4: ...
    @staticmethod
    def from_rotation_y(angle: float) -> Mat4: ...
    @staticmethod
    def from_rotation_z(angle: float) -> Mat4: ...
    @staticmethod
    def from_scale_rotation_translation(scale: Vec3, rotation: Quat, translation: Vec3) -> Mat4: ...
    @staticmethod
    def from_mat3(mat3: Mat3) -> Mat4: ...
    @staticmethod
    def perspective_lh(fov_y_radians: float, aspect_ratio: float, z_near: float, z_far: float) -> Mat4:
        """Create a perspective projection.

        Raises:
            ValueError: Unless z_near > 0 and z_far > 0, or if the parameters
                produce a non-finite matrix (zero field of view or aspect ratio,
                equal depth planes, or NaN input)
        """
    @staticmethod
    def perspective_rh(fov_y_radians: float, aspect_ratio: float, z_near: float, z_far: float) -> Mat4:
        """Create a perspective projection.

        Raises:
            ValueError: Unless z_near > 0 and z_far > 0, or if the parameters
                produce a non-finite matrix (zero field of view or aspect ratio,
                equal depth planes, or NaN input)
        """

    @staticmethod
    def perspective_infinite_reverse_rh(
        fov_y_radians: float, aspect_ratio: float, z_near: float
    ) -> Mat4:
        """Create a right-handed perspective with an infinite far plane and reversed depth.

        This matches Bevy's rendering convention: the near plane maps to NDC 1
        and infinity approaches 0.

        Raises `ValueError` unless `z_near > 0`, or if the parameters produce a
        non-finite matrix.
        """

    @staticmethod
    def orthographic_lh(left: float, right: float, bottom: float, top: float, near: float, far: float) -> Mat4:
        """Create an orthographic projection.

        Raises:
            ValueError: If the parameters produce a non-finite matrix, such as
                equal opposite bounds or non-finite input
        """
    @staticmethod
    def orthographic_rh(left: float, right: float, bottom: float, top: float, near: float, far: float) -> Mat4:
        """Create an orthographic projection.

        Raises:
            ValueError: If the parameters produce a non-finite matrix, such as
                equal opposite bounds or non-finite input
        """
    @staticmethod
    def look_at_lh(eye: Vec3, center: Vec3, up: Vec3) -> Mat4:
        """Build a view matrix looking from eye toward center.

        Raises:
            ValueError: If eye and center coincide or the view direction is parallel to up
        """
    @staticmethod
    def look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Mat4:
        """Build a view matrix looking from eye toward center.

        Raises:
            ValueError: If eye and center coincide or the view direction is parallel to up
        """
    @staticmethod
    def look_to_lh(eye: Vec3, dir: Vec3, up: Vec3) -> Mat4:
        """Build a left-handed view; dir and up must be non-parallel unit vectors or ValueError is raised."""
    @staticmethod
    def look_to_rh(eye: Vec3, dir: Vec3, up: Vec3) -> Mat4:
        """Build a right-handed view; dir and up must be non-parallel unit vectors or ValueError is raised."""

    def col(self, index: int) -> Vec4:
        """The matrix's column at `index`, counting from the end if negative.

        Raises:
            IndexError: If the index is outside -4 to 3
        """
    def row(self, index: int) -> Vec4:
        """The matrix's row at `index`, counting from the end if negative.

        Raises:
            IndexError: If the index is outside -4 to 3
        """
    def to_cols_array(self) -> list[float]: ...
    def to_cols_array_2d(self) -> list[list[float]]: ...
    def transpose(self) -> Mat4: ...
    def determinant(self) -> float: ...
    def inverse(self) -> Mat4:
        """Return the inverse matrix.

        Raises:
            ValueError: If the matrix is singular or its inverse is not finite
        """
    def mul_vec4(self, rhs: Vec4) -> Vec4: ...
    def mul_mat4(self, rhs: Mat4) -> Mat4: ...
    def add_mat4(self, rhs: Mat4) -> Mat4: ...
    def sub_mat4(self, rhs: Mat4) -> Mat4: ...
    def mul_scalar(self, rhs: float) -> Mat4: ...
    def div_scalar(self, rhs: float) -> Mat4: ...
    def transform_point3(self, rhs: Vec3) -> Vec3: ...
    def transform_vector3(self, rhs: Vec3) -> Vec3: ...
    def project_point3(self, rhs: Vec3) -> Vec3: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    def abs_diff_eq(self, rhs: Mat4, max_abs_diff: float) -> bool: ...

    def __add__(self, other: Mat4) -> Mat4: ...
    def __sub__(self, other: Mat4) -> Mat4: ...
    @overload
    def __mul__(self, other: float) -> Mat4: ...
    @overload
    def __mul__(self, other: Mat4) -> Mat4: ...
    @overload
    def __mul__(self, other: Vec4) -> Vec4: ...
    def __rmul__(self, other: float) -> Mat4: ...
    def __truediv__(self, scalar: float) -> Mat4: ...
    def __neg__(self) -> Mat4: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> Mat4:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Mat4:
        """Return an independent copy; the value holds no Python objects."""

    @property
    def x_axis(self) -> Vec4:
        """The first column; nested writes reach this matrix."""
    @x_axis.setter
    def x_axis(self, value: Vec4) -> None: ...
    @property
    def y_axis(self) -> Vec4:
        """The second column; nested writes reach this matrix."""
    @y_axis.setter
    def y_axis(self, value: Vec4) -> None: ...
    @property
    def z_axis(self) -> Vec4:
        """The third column; nested writes reach this matrix."""
    @z_axis.setter
    def z_axis(self, value: Vec4) -> None: ...
    @property
    def w_axis(self) -> Vec4:
        """The fourth column; nested writes reach this matrix."""
    @w_axis.setter
    def w_axis(self, value: Vec4) -> None: ...
    def to_scale_rotation_translation(self) -> tuple[Vec3, Quat, Vec3]:
        """Decompose into scale, rotation, and translation.

        Raises:
            ValueError: If the determinant is zero or not finite.
        """
    @staticmethod
    def from_euler(order: EulerRot, a: float, b: float, c: float) -> Mat4: ...
    def to_euler(self, order: EulerRot) -> tuple[float, float, float]:
        """Extract Euler angles in the given order.

        Raises:
            ValueError: If the x, y, or z column is not normalized.
        """
    def abs(self) -> Mat4: ...
    @staticmethod
    def from_rotation_translation(rotation: Quat, translation: Vec3) -> Mat4: ...
    def transform_point3a(self, rhs: Vec3A) -> Vec3A:
        """Transform a point, applying translation.

        Raises:
            ValueError: If the last row is not (0, 0, 0, 1).
        """
    def transform_vector3a(self, rhs: Vec3A) -> Vec3A:
        """Transform a vector, ignoring translation.

        Raises:
            ValueError: If the last row is not (0, 0, 0, 1).
        """
    @staticmethod
    def perspective_infinite_lh(fov_y_radians: float, aspect_ratio: float, z_near: float) -> Mat4:
        """Left-handed infinite perspective projection.

        Raises:
            ValueError: If `z_near` is not positive or the matrix is not finite.
        """
    @staticmethod
    def perspective_infinite_rh(fov_y_radians: float, aspect_ratio: float, z_near: float) -> Mat4:
        """Right-handed infinite perspective projection.

        Raises:
            ValueError: If `z_near` is not positive or the matrix is not finite.
        """
    @staticmethod
    def perspective_infinite_reverse_lh(
        fov_y_radians: float, aspect_ratio: float, z_near: float
    ) -> Mat4:
        """Left-handed infinite reverse-Z perspective projection.

        Raises:
            ValueError: If `z_near` is not positive or the matrix is not finite.
        """

class Isometry2d:
    """2D isometry representing translation and rotation (rigid transform)."""

    IDENTITY: ClassVar[Isometry2d]

    @property
    def translation(self) -> Vec2: ...
    @translation.setter
    def translation(self, value: Vec2) -> None: ...
    @property
    def rotation(self) -> Rot2: ...
    @rotation.setter
    def rotation(self, value: Rot2) -> None: ...

    def __init__(
        self, translation: Vec2 = Vec2.ZERO, rotation: Rot2 = ...
    ) -> None:
        """Create a 2D isometry.

        Args:
            translation: Translation vector (default is Vec2.ZERO)
            rotation: Rotation as Rot2 (default is identity rotation)
        """

    @staticmethod
    def from_rotation(rotation: Rot2) -> Isometry2d:
        """Create a 2D isometry from a rotation only."""

    @staticmethod
    def from_translation(translation: Vec2) -> Isometry2d:
        """Create a 2D isometry from a translation only."""

    @staticmethod
    def from_xy(x: float, y: float) -> Isometry2d:
        """Create a 2D isometry from x and y translation components."""

    def inverse(self) -> Isometry2d:
        """Get the inverse isometry that undoes this transformation."""

    def inverse_mul(self, rhs: Isometry2d) -> Isometry2d:
        """Compute self.inverse() * rhs efficiently for one-shot cases."""

    def transform_point(self, point: Vec2) -> Vec2:
        """Transform a point by rotating and translating it."""

    def inverse_transform_point(self, point: Vec2) -> Vec2:
        """Transform a point using the inverse of this isometry."""

    @overload
    def __mul__(self, other: Isometry2d) -> Isometry2d: ...
    @overload
    def __mul__(self, other: Vec2) -> Vec2: ...
    @overload
    def __mul__(self, other: Dir2) -> Dir2: ...

    def __eq__(self, other: object) -> bool: ...

    def __copy__(self) -> Isometry2d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Isometry2d:
        """Return an independent copy; the value holds no Python objects."""

class Isometry3d:
    """3D isometry (rotation + translation) for bounding volume transformations.

    An isometry represents a rigid transformation that preserves distances -
    a combination of rotation and translation, without any scaling.
    """

    IDENTITY: ClassVar[Isometry3d]

    translation: Vec3A
    rotation: Quat

    def __init__(self, translation: Vec3A | Vec3 = ..., rotation: Quat = ...) -> None:
        """Create a new Isometry3d.

        Args:
            translation: The translation component (default: Vec3.ZERO)
            rotation: The rotation component (default: Quat.IDENTITY)
        """

    @staticmethod
    def from_xyz(x: float, y: float, z: float) -> Isometry3d:
        """Create an isometry from translation coordinates with identity rotation."""

    @staticmethod
    def from_translation(translation: Vec3A | Vec3) -> Isometry3d:
        """Create an isometry from a translation vector with identity rotation."""

    @staticmethod
    def from_rotation(rotation: Quat) -> Isometry3d:
        """Create an isometry from a rotation with zero translation."""

    def inverse(self) -> Isometry3d:
        """Get the inverse of this isometry."""

    def inverse_mul(self, rhs: Isometry3d) -> Isometry3d:
        """Compute the equivalent of applying the inverse of self followed by rhs."""

    def transform_point(self, point: Vec3A | Vec3) -> Vec3:
        """Transform a point by this isometry (rotate then translate)."""

    def inverse_transform_point(self, point: Vec3A | Vec3) -> Vec3:
        """Inverse-transform a point (undo translation then undo rotation)."""

    @overload
    def __mul__(self, other: Isometry3d) -> Isometry3d: ...
    @overload
    def __mul__(self, other: Vec3A) -> Vec3A: ...
    @overload
    def __mul__(self, other: Vec3) -> Vec3: ...
    @overload
    def __mul__(self, other: Dir3) -> Dir3: ...

    def __copy__(self) -> Isometry3d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Isometry3d:
        """Return an independent copy; the value holds no Python objects."""

class Dir2:
    """A normalized 2D direction vector."""

    X: ClassVar[Dir2]
    Y: ClassVar[Dir2]
    NEG_X: ClassVar[Dir2]
    NEG_Y: ClassVar[Dir2]
    NORTH: ClassVar[Dir2]
    SOUTH: ClassVar[Dir2]
    EAST: ClassVar[Dir2]
    WEST: ClassVar[Dir2]
    NORTH_EAST: ClassVar[Dir2]
    NORTH_WEST: ClassVar[Dir2]
    SOUTH_EAST: ClassVar[Dir2]
    SOUTH_WEST: ClassVar[Dir2]

    @property
    def x(self) -> float: ...

    @property
    def y(self) -> float: ...

    def __init__(self, x: float, y: float) -> None:
        """Create a normalized direction from x, y components.

        Raises:
            ValueError: If the vector length is zero, very close to zero, or non-finite
        """

    @staticmethod
    def from_vec2(vec: Vec2) -> Dir2:
        """Create a normalized direction from a Vec2.

        Raises:
            ValueError: If the vector length is zero, very close to zero, or non-finite
        """

    def as_vec2(self) -> Vec2:
        """Convert to Vec2."""

    def dot(self, other: Dir2) -> float:
        """Compute dot product with another direction."""

    def perp(self) -> Dir2:
        """Get perpendicular direction (rotated 90 degrees counterclockwise)."""

    def slerp(self, rhs: Dir2, s: float) -> Dir2:
        """Spherical linear interpolation between two directions."""


    def rotation_to(self, other: Dir2) -> Rot2:
        """Get the rotation from self to other."""

    def rotation_from(self, other: Dir2) -> Rot2:
        """Get the rotation from other to self."""

    def rotation_from_x(self) -> Rot2:
        """Get the rotation from the X axis to self."""

    def rotation_to_x(self) -> Rot2:
        """Get the rotation from self to the X axis."""

    def rotation_from_y(self) -> Rot2:
        """Get the rotation from the Y axis to self."""

    def rotation_to_y(self) -> Rot2:
        """Get the rotation from self to the Y axis."""

    def fast_renormalize(self) -> Dir2:
        """Quickly renormalize using a first-order Taylor approximation."""

    @staticmethod
    def from_xy_unchecked(x: float, y: float) -> Dir2:
        """Create a direction without checking if it's normalized."""

    @staticmethod
    def new_unchecked(value: Vec2) -> Dir2:
        """Create a direction from a Vec2 without checking normalization."""

    def interpolate_stable(self, other: Dir2, t: float) -> Dir2:
        """Stable interpolation between two directions."""

    def interpolate_stable_assign(self, other: Dir2, t: float) -> None:
        """Stable interpolation, assigning result to self."""

    def smooth_nudge(self, target: Dir2, decay_rate: float, delta: float) -> None:
        """Smoothly nudge towards target at given decay rate."""

    def __neg__(self) -> Dir2:
        """Negate direction."""

    def __mul__(self, scalar: float) -> Vec2:
        """Multiply direction by scalar to get vector."""

    def __rmul__(self, scalar: float) -> Vec2:
        """Multiply scalar by direction to get vector."""
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> Dir2:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Dir2:
        """Return an independent copy; the value holds no Python objects."""

    @staticmethod
    def from_xy(x: float, y: float) -> Dir2:
        """Create a direction from x and y components.

        Raises:
            ValueError: If the vector length is zero, very close to zero, or non-finite
        """
    @staticmethod
    def new_and_length(value: Vec2) -> tuple[Dir2, float]:
        """Normalize `value`, returning the direction and the original length.

        Raises:
            ValueError: If the vector length is zero, very close to zero, or non-finite
        """

class CompassOctant:
    """Eight compass directions.

    Represents the eight ordinal and cardinal directions on a compass.
    Useful for grid-based movement, UI layout, or spatial queries.

    Example:
        ```python
        from pybevy.math import CompassOctant

        direction = CompassOctant.NorthEast  # Moving diagonally up-right
        ```
    """

    def __copy__(self) -> CompassOctant: ...
    def __deepcopy__(self, memo: dict[int, object]) -> CompassOctant: ...

    North: CompassOctant
    """North (up)."""

    NorthEast: CompassOctant
    """North-East (up-right diagonal)."""

    East: CompassOctant
    """East (right)."""

    SouthEast: CompassOctant
    """South-East (down-right diagonal)."""

    South: CompassOctant
    """South (down)."""

    SouthWest: CompassOctant
    """South-West (down-left diagonal)."""

    West: CompassOctant
    """West (left)."""

    NorthWest: CompassOctant
    """North-West (up-left diagonal)."""

    def is_in_direction(self, origin: Vec2, candidate: Vec2) -> bool:
        """Check if a candidate point is in this compass direction from an origin point."""

    def __neg__(self) -> CompassOctant: ...

    def opposite(self) -> CompassOctant:
        """Get the opposite compass direction."""

    @staticmethod
    def from_index(index: int) -> CompassOctant | None:
        """Get the compass direction from an index (0-7), or None if out of range."""

    def to_index(self) -> int:
        """Get the index (0-7) of this compass direction."""

    def __hash__(self) -> int: ...


class CompassQuadrant:
    """Four cardinal compass directions.

    Represents the four cardinal directions on a compass (N, E, S, W).
    Useful for grid-based movement, UI layout, or spatial queries.

    Example:
        ```python
        from pybevy.math import CompassQuadrant

        direction = CompassQuadrant.North  # Moving up
        ```
    """

    def __copy__(self) -> CompassQuadrant: ...
    def __deepcopy__(self, memo: dict[int, object]) -> CompassQuadrant: ...

    North: CompassQuadrant
    """North (up), corresponds to Dir2::Y."""

    East: CompassQuadrant
    """East (right), corresponds to Dir2::X."""

    South: CompassQuadrant
    """South (down), corresponds to Dir2::NEG_Y."""

    West: CompassQuadrant
    """West (left), corresponds to Dir2::NEG_X."""

    def is_in_direction(self, origin: Vec2, candidate: Vec2) -> bool:
        """Check for a positive projection from the origin onto this direction."""

    def __neg__(self) -> CompassQuadrant: ...

    def opposite(self) -> CompassQuadrant:
        """Get the opposite compass direction."""

    @staticmethod
    def from_index(index: int) -> CompassQuadrant | None:
        """Get the compass direction from an index (0-3), or None if out of range."""

    def to_index(self) -> int:
        """Get the index (0-3) of this compass direction."""

    def __hash__(self) -> int: ...


class Rot2:
    """A 2D rotation represented as a unit complex number.

    Rot2 stores the cosine and sine of the rotation angle, enabling
    efficient rotation operations without trigonometric calculations.
    Angle getters return a signed principal angle, so they need not return
    the original constructor argument. For example, a 0.75-turn rotation
    reads back as -0.25 turn; both describe the same rotation.

    Example:
        ```python
        from pybevy.math import Rot2, Vec2
        import math

        # Create rotations
        rot = Rot2.degrees(45)
        rot = Rot2.radians(math.pi / 4)
        rot = Rot2.turn_fraction(0.125)  # 1/8 turn = 45 degrees

        # Rotate a vector
        v = Vec2(1, 0)
        rotated = rot * v

        # Combine rotations
        double_rot = rot * rot

        # Interpolate rotations
        halfway = Rot2.IDENTITY.slerp(Rot2.FRAC_PI_2, 0.5)
        ```
    """

    IDENTITY: ClassVar[Rot2]
    PI: ClassVar[Rot2]
    FRAC_PI_2: ClassVar[Rot2]
    FRAC_PI_3: ClassVar[Rot2]
    FRAC_PI_4: ClassVar[Rot2]
    FRAC_PI_6: ClassVar[Rot2]
    FRAC_PI_8: ClassVar[Rot2]

    @property
    def cos(self) -> float:
        """Get the cosine of the rotation angle."""

    @property
    def sin(self) -> float:
        """Get the sine of the rotation angle."""

    def __init__(self, *, cos: float = ..., sin: float = ...) -> None:
        """Create a rotation from cos/sin values.

        Args:
            cos: Cosine of the angle
            sin: Sine of the angle

        Omitting both returns identity (cos=1, sin=0); otherwise both are required.
        Non-finite or non-unit pairs raise ValueError. Explicit None is invalid.
        """

    @staticmethod
    def radians(radians: float) -> Rot2:
        """Create a rotation from an angle in radians."""

    @staticmethod
    def degrees(degrees: float) -> Rot2:
        """Create a rotation from an angle in degrees."""

    @staticmethod
    def turn_fraction(fraction: float) -> Rot2:
        """Create a rotation from a fraction of a full turn.

        Args:
            fraction: Fraction of a full turn (0.5 represents a half-turn).

        Readback uses a signed principal fraction, not necessarily this input.
        """

    @staticmethod
    def from_sin_cos(sin: float, cos: float) -> Rot2:
        """Create a rotation from sine and cosine values.

        Note: The values should form a unit vector (sin² + cos² = 1).
        """

    def as_radians(self) -> float:
        """Get a signed principal angle, approximately in [-pi, pi] radians.

        A half-turn lies on the branch cut and may use either signed endpoint.
        """

    def as_degrees(self) -> float:
        """Get a signed principal angle, approximately in [-180, 180] degrees.

        A half-turn lies on the branch cut and may use either signed endpoint.
        """

    def as_turn_fraction(self) -> float:
        """Get a signed principal fraction, approximately in [-0.5, 0.5].

        This is not an inverse of turn_fraction for arguments outside the
        principal range: 0.75 turn reads back as -0.25 turn. A half-turn may
        use either signed endpoint due to floating-point rounding.
        """

    def inverse(self) -> Rot2:
        """Get the inverse rotation (negated angle)."""

    def is_normalized(self) -> bool:
        """Check if the rotation is normalized (unit length)."""

    def normalize(self) -> Rot2:
        """Return a normalized copy of this rotation."""

    def sin_cos(self) -> tuple[float, float]:
        """Return the sine and cosine of the rotation angle as a tuple (sin, cos)."""

    def length(self) -> float:
        """Return the length of the rotation's underlying complex number representation."""

    def length_squared(self) -> float:
        """Return the squared length of the rotation's underlying complex number representation."""

    def length_recip(self) -> float:
        """Return the reciprocal of the length."""

    def try_normalize(self) -> Rot2 | None:
        """Attempt to normalize the rotation, returning None if the length is too small."""

    def fast_renormalize(self) -> Rot2:
        """Fast approximate renormalization using a first-order Taylor approximation."""

    def is_near_identity(self) -> bool:
        """Check if this rotation is near the identity rotation."""

    def angle_to(self, other: Rot2) -> float:
        """Get the angle between this rotation and another in radians."""

    def slerp(self, rhs: Rot2, s: float) -> Rot2:
        """Spherical linear interpolation between rotations.

        Args:
            rhs: Target rotation
            s: Interpolation factor (0.0 = self, 1.0 = rhs)
        """

    def nlerp(self, rhs: Rot2, s: float) -> Rot2:
        """Normalized linear interpolation between rotations.

        Faster but less accurate than slerp for small angles.

        Args:
            rhs: Target rotation
            s: Interpolation factor (0.0 = self, 1.0 = rhs)
        """

    def is_finite(self) -> bool:
        """Check if the rotation values are finite."""

    def is_nan(self) -> bool:
        """Check if any rotation values are NaN."""

    def interpolate_stable(self, other: Rot2, t: float) -> Rot2:
        """Stable interpolation between two rotations."""

    def interpolate_stable_assign(self, other: Rot2, t: float) -> None:
        """Stable interpolation, assigning result to self."""

    def smooth_nudge(self, target: Rot2, decay_rate: float, delta: float) -> None:
        """Smoothly nudge towards target at given decay rate."""

    @overload
    def __mul__(self, other: Rot2) -> Rot2:
        """Compose two rotations."""

    @overload
    def __mul__(self, other: Vec2) -> Vec2:
        """Rotate a 2D vector."""
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

    def __copy__(self) -> Rot2:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Rot2:
        """Return an independent copy; the value holds no Python objects."""
