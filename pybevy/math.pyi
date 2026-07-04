from collections.abc import Sequence
from typing import Any, ClassVar, overload

from pybevy.mesh import (
    AnnulusMeshBuilder,
    Capsule2dMeshBuilder,
    Capsule3dMeshBuilder,
    CircleMeshBuilder,
    CircularSectorMeshBuilder,
    CircularSegmentMeshBuilder,
    ConeMeshBuilder,
    CuboidMeshBuilder,
    CylinderMeshBuilder,
    EllipseMeshBuilder,
    Meshable,
    PlaneMeshBuilder,
    RectangleMeshBuilder,
    RegularPolygonMeshBuilder,
    RhombusMeshBuilder,
    Segment2dMeshBuilder,
    SphereMeshBuilder,
    TetrahedronMeshBuilder,
    TorusMeshBuilder,
    Triangle2dMeshBuilder,
    Triangle3dMeshBuilder,
)

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

    def __init__(self, x: float, y: float, z: float) -> None:
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
    def __div__(self, other: float | Vec3) -> Vec3: ...
    def __truediv__(self, other: float | Vec3) -> Vec3: ...
    def __neg__(self) -> Vec3: ...
    def __rmul__(self, other: float | Vec3) -> Vec3: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def as_tuple(self) -> tuple[float, float, float]: ...
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
    def round(self) -> Vec3: ...
    def trunc(self) -> Vec3: ...
    def fract(self) -> Vec3: ...
    def fract_gl(self) -> Vec3: ...
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
    def any_orthonormal_vector(self) -> Vec3: ...
    def any_orthonormal_pair(self) -> tuple[Vec3, Vec3]: ...
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
        """
    def reject_from_normalized(self, rhs: Vec3) -> Vec3:
        """Rejects this vector from a normalized vector.

        More efficient than reject_from when rhs is already normalized.
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

    def __init__(self, x: float, y: float, z: float) -> None: ...
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
    def abs(self) -> Vec3A: ...
    def min(self, other: Vec3A) -> Vec3A: ...
    def max(self, other: Vec3A) -> Vec3A: ...
    def distance(self, other: Vec3A) -> float: ...
    def distance_squared(self, other: Vec3A) -> float: ...
    def lerp(self, other: Vec3A, t: float) -> Vec3A: ...
    def __add__(self, other: Vec3A) -> Vec3A: ...
    def __sub__(self, other: Vec3A) -> Vec3A: ...
    def __mul__(self, scalar: float) -> Vec3A: ...
    def __rmul__(self, scalar: float) -> Vec3A: ...
    def __truediv__(self, scalar: float) -> Vec3A: ...
    def __neg__(self) -> Vec3A: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

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

    def __init__(self, x: float, y: float) -> None: ...
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
    def round(self) -> Vec2: ...
    def trunc(self) -> Vec2: ...
    def fract(self) -> Vec2: ...
    def fract_gl(self) -> Vec2: ...
    def exp(self) -> Vec2: ...
    def powf(self, n: float) -> Vec2: ...
    def recip(self) -> Vec2: ...
    def lerp(self, rhs: Vec2, s: float) -> Vec2: ...
    def move_towards(self, rhs: Vec2, d: float) -> Vec2: ...
    def midpoint(self, rhs: Vec2) -> Vec2: ...
    def project_onto(self, rhs: Vec2) -> Vec2: ...
    def reject_from(self, rhs: Vec2) -> Vec2: ...
    def project_onto_normalized(self, rhs: Vec2) -> Vec2: ...
    def reject_from_normalized(self, rhs: Vec2) -> Vec2: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    def cmpeq(self, rhs: Vec2) -> tuple[bool, bool]: ...
    def cmpne(self, rhs: Vec2) -> tuple[bool, bool]: ...
    def cmpge(self, rhs: Vec2) -> tuple[bool, bool]: ...
    def cmpgt(self, rhs: Vec2) -> tuple[bool, bool]: ...
    def cmple(self, rhs: Vec2) -> tuple[bool, bool]: ...
    def cmplt(self, rhs: Vec2) -> tuple[bool, bool]: ...
    def angle_between(self, other: Vec2) -> float: ...
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
    def __div__(self, other: float | Vec2) -> Vec2: ...
    def __truediv__(self, other: float | Vec2) -> Vec2: ...
    def __neg__(self) -> Vec2: ...
    def __rmul__(self, other: float | Vec2) -> Vec2: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def as_tuple(self) -> tuple[float, float]: ...

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
        """Returns True if the range contains no items."""

    def contains(self, value: float) -> bool:
        """Returns True if value is within the range [start, end)."""

class Rect:
    """A rectangle defined by minimum and maximum corner points."""

    min: Vec2
    max: Vec2

    def __init__(self, x0: float, y0: float, x1: float, y1: float) -> None:
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
        """Create a Rect from center point and size."""

    @staticmethod
    def from_center_half_size(origin: Vec2, half_size: Vec2) -> Rect:
        """Create a Rect from center point and half size."""

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

    def __init__(self, x: int, y: int) -> None:
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

    def __add__(self, other: UVec2) -> UVec2: ...
    def __sub__(self, other: UVec2) -> UVec2: ...
    def __mul__(self, scalar: int) -> UVec2: ...
    def __truediv__(self, scalar: int) -> UVec2: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

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

    def __init__(self, x: int, y: int) -> None:
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

    def __add__(self, other: IVec2) -> IVec2: ...
    def __sub__(self, other: IVec2) -> IVec2: ...
    def __mul__(self, scalar: int) -> IVec2: ...
    def __truediv__(self, scalar: int) -> IVec2: ...
    def __neg__(self) -> IVec2: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

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

    def __init__(self, x: int, y: int, z: int) -> None:
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

    def __add__(self, other: UVec3) -> UVec3: ...
    def __sub__(self, other: UVec3) -> UVec3: ...
    def __mul__(self, scalar: int) -> UVec3: ...
    def __truediv__(self, scalar: int) -> UVec3: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class URect:
    """A rectangle defined by minimum and maximum corner points (unsigned integers)."""

    min: UVec2
    max: UVec2

    EMPTY: ClassVar[URect]

    def __init__(self, x0: int, y0: int, x1: int, y1: int) -> None:
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
        """Create a URect from center point and size."""

    @staticmethod
    def from_center_half_size(origin: UVec2, half_size: UVec2) -> URect:
        """Create a URect from center point and half size."""

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

class Circle(Meshable):
    def __init__(self, radius: float = 0.5) -> None: ...
    @property
    def radius(self) -> float: ...
    @radius.setter
    def radius(self, value: float) -> None: ...
    def diameter(self) -> float: ...
    def area(self) -> float: ...
    def perimeter(self) -> float: ...
    def closest_point(self, point: Vec2) -> Vec2: ...
    def mesh(self) -> CircleMeshBuilder: ...

class EulerRot:
    """Euler rotation order for converting to/from Euler angles.

    Variants represent different rotation orders (intrinsic three-axis rotations).
    """
    ZYX: EulerRot  # Intrinsic three-axis rotation ZYX
    ZXY: EulerRot  # Intrinsic three-axis rotation ZXY
    YXZ: EulerRot  # Intrinsic three-axis rotation YXZ
    YZX: EulerRot  # Intrinsic three-axis rotation YZX
    XYZ: EulerRot  # Intrinsic three-axis rotation XYZ
    XZY: EulerRot  # Intrinsic three-axis rotation XZY

class Quat:
    IDENTITY: ClassVar[Quat]
    NAN: ClassVar[Quat]

    x: float
    y: float
    z: float
    w: float

    def __init__(self, x: float, y: float, z: float, w: float) -> None: ...
    @staticmethod
    def from_xyzw(x: float, y: float, z: float, w: float) -> Quat: ...
    @staticmethod
    def from_axis_angle(axis: Vec3, angle: float) -> Quat: ...
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
    def normalize(self) -> Quat: ...
    def conjugate(self) -> Quat: ...
    def inverse(self) -> Quat: ...
    def lerp(self, rhs: Quat, s: float) -> Quat:
        """Linear interpolation between two quaternions."""
    def slerp(self, rhs: Quat, s: float) -> Quat:
        """Spherical linear interpolation between two quaternions."""
    @staticmethod
    def from_euler(order: EulerRot, x: float, y: float, z: float) -> Quat:
        """Create a quaternion from Euler angles.

        Args:
            order: The rotation order (e.g., EulerRot.YXZ)
            x: Rotation around X axis in radians
            y: Rotation around Y axis in radians
            z: Rotation around Z axis in radians

        Returns:
            Quat: The resulting quaternion
        """
    def to_euler(self, order: EulerRot) -> tuple[float, float, float]:
        """Convert quaternion to Euler angles.

        Args:
            order: The rotation order (e.g., EulerRot.YXZ)

        Returns:
            tuple: (x, y, z) rotations in radians
        """
    @overload
    def __mul__(self, other: Quat) -> Quat: ...
    @overload
    def __mul__(self, other: Vec3) -> Vec3: ...
    def __rmul__(self, other: Quat) -> Quat: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

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
    def cross(self, other: Dir3) -> Dir3: ...
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
    def __richcmp__(self, other: Any, op: Any) -> bool: ...
    def as_tuple(self) -> tuple[float, float, float]: ...
    def __neg__(self) -> Dir3: ...
    def __mul__(self, other: float) -> Vec3: ...
    def __rmul__(self, other: float) -> Vec3: ...

class Mat2:
    """2x2 column-major matrix for rotation/scale/shear."""

    IDENTITY: ClassVar[Mat2]
    ZERO: ClassVar[Mat2]
    NAN: ClassVar[Mat2]
    def __init__(
        self, x_axis: Vec2 | None = None, y_axis: Vec2 | None = None
    ) -> None: ...
    @staticmethod
    def from_cols(x_axis: Vec2, y_axis: Vec2) -> Mat2: ...
    @staticmethod
    def from_angle(angle: float) -> Mat2: ...
    @staticmethod
    def from_scale_angle(scale: Vec2, angle: float) -> Mat2: ...
    @staticmethod
    def from_diagonal(diagonal: Vec2) -> Mat2: ...
    def col(self, index: int) -> Vec2: ...
    def row(self, index: int) -> Vec2: ...
    @property
    def x_axis(self) -> Vec2: ...
    @property
    def y_axis(self) -> Vec2: ...
    def transpose(self) -> Mat2: ...
    def determinant(self) -> float: ...
    def inverse(self) -> Mat2: ...
    def mul_vec2(self, rhs: Vec2) -> Vec2: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    def __mul__(self, other: Mat2 | float) -> Mat2: ...
    def __add__(self, other: Mat2) -> Mat2: ...
    def __sub__(self, other: Mat2) -> Mat2: ...
    def __neg__(self) -> Mat2: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class Affine2:
    """2D affine transformation (scale, shear, rotation, translation)."""

    IDENTITY: ClassVar[Affine2]
    ZERO: ClassVar[Affine2]
    NAN: ClassVar[Affine2]
    def __init__(
        self, matrix2: Mat2 | None = None, translation: Vec2 | None = None
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
    def inverse(self) -> Affine2: ...
    def transform_point2(self, point: Vec2) -> Vec2: ...
    def transform_vector2(self, vector: Vec2) -> Vec2: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    def into_mat3(self) -> Mat3: ...
    def __mul__(self, other: Affine2) -> Affine2: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class Affine3A:
    """A 3D affine transform using SIMD-aligned types.

    Composed of a 3x3 matrix (rotation/scale/shear) and a translation vector.
    """

    IDENTITY: ClassVar[Affine3A]
    ZERO: ClassVar[Affine3A]
    NAN: ClassVar[Affine3A]

    def __init__(
        self, matrix3: Mat3A | None = None, translation: Vec3A | None = None
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
    @property
    def translation(self) -> Vec3A: ...

    def transform_point3(self, rhs: Vec3) -> Vec3: ...
    def transform_point3a(self, rhs: Vec3A) -> Vec3A: ...
    def transform_vector3(self, rhs: Vec3) -> Vec3: ...
    def transform_vector3a(self, rhs: Vec3A) -> Vec3A: ...
    def inverse(self) -> Affine3A: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...

    def __mul__(self, other: Affine3A) -> Affine3A: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class Cone(Meshable):
    def __init__(self, radius: float = 0.5, height: float = 1.0) -> None: ...
    @property
    def radius(self) -> float: ...
    @radius.setter
    def radius(self, value: float) -> None: ...
    @property
    def height(self) -> float: ...
    @height.setter
    def height(self, value: float) -> None: ...
    def base(self) -> Circle: ...
    def base_area(self) -> float: ...
    def lateral_area(self) -> float: ...
    def slant_height(self) -> float: ...
    def area(self) -> float: ...
    def volume(self) -> float: ...
    def mesh(self) -> ConeMeshBuilder: ...

class Rectangle(Meshable):
    def __init__(
        self,
        width: float = 1.0,
        height: float = 1.0,
        *,
        half_size: Vec2 | None = None,
    ) -> None: ...
    @staticmethod
    def from_size(size: Vec2) -> Rectangle: ...
    @staticmethod
    def from_corners(point1: Vec2, point2: Vec2) -> Rectangle: ...
    @staticmethod
    def from_length(length: float) -> Rectangle: ...
    @property
    def half_size(self) -> Vec2: ...
    @half_size.setter
    def half_size(self, value: Vec2) -> None: ...
    def size(self) -> Vec2: ...
    def closest_point(self, point: Vec2) -> Vec2: ...
    def area(self) -> float: ...
    def perimeter(self) -> float: ...
    def mesh(self) -> RectangleMeshBuilder: ...

class Sphere(Meshable):
    def __init__(self, radius: float = 0.5) -> None: ...
    @property
    def radius(self) -> float: ...
    @radius.setter
    def radius(self, value: float) -> None: ...
    def diameter(self) -> float: ...
    def area(self) -> float: ...
    def volume(self) -> float: ...
    def closest_point(self, point: Vec3) -> Vec3: ...
    def mesh(self) -> SphereMeshBuilder: ...  # type: ignore[override]

class Plane3d(Meshable):
    """A finite 3D plane (quad) defined by a normal direction and half-size.

    The ``half_size`` Vec2 maps to the plane's local 2D axes, which depend on
    the normal:

    - ``Plane3d(Vec3.Y, half_size=Vec2(w, h))`` — horizontal plane.
      ``w`` → world X, ``h`` → world Z.
    - ``Plane3d(Vec3(0,0,1), half_size=Vec2(w, h))`` — vertical plane facing +Z.
      ``w`` → world X (width), ``h`` → world Y (height).
      Use this for screens / billboards facing the camera.

    Example::

        # Landscape screen, 6.4 wide x 2.6 tall, facing +Z
        screen = meshes.add(Plane3d(Vec3(0.0, 0.0, 1.0), half_size=Vec2(3.2, 1.3)))
    """

    def __init__(
        self, normal: Vec3 = Dir3.Y.as_vec3(), half_size: Vec2 = Vec2.splat(0.5)
    ) -> None: ...
    @staticmethod
    def from_points(a: Vec3, b: Vec3, c: Vec3) -> tuple[Plane3d, Vec3]:
        """Create a plane from three points, returning the plane and translation."""
    @property
    def half_size(self) -> Vec2: ...
    @half_size.setter
    def half_size(self, value: Vec2) -> None: ...
    def mesh(self) -> PlaneMeshBuilder: ...

class InfinitePlane3d:
    """An infinite 3D plane defined by its normal direction."""

    def __init__(self, normal: Vec3 = Dir3.Y.as_vec3()) -> None: ...
    @staticmethod
    def from_dir(normal: Dir3) -> InfinitePlane3d:
        """Create an infinite plane from a direction."""
    @property
    def normal(self) -> Dir3: ...

class HalfSpace:
    """A region of 3D space defined by a bisecting 2D plane.

    The first 3 components of the Vec4 represent the plane's unit normal,
    and the 4th component is the signed distance from the plane to the origin.
    """

    @property
    def normal(self) -> Vec4: ...
    @property
    def d(self) -> float: ...
    @property
    def normal_d(self) -> Vec4: ...

    def __init__(self, normal_d: Vec4) -> None: ...

class ViewFrustum:
    """A region of 3D space defined by the intersection of 6 half-spaces.

    Half spaces are ordered left, right, top, bottom, near, far; their normals
    point towards the interior of the frustum. Wrap in a `Frustum` component
    (pybevy.camera) to attach it to an entity.
    """

    NEAR_PLANE_IDX: ClassVar[int]
    FAR_PLANE_IDX: ClassVar[int]

    @property
    def half_spaces(self) -> list[HalfSpace]: ...
    @half_spaces.setter
    def half_spaces(self, value: list[HalfSpace]) -> None: ...

    def __init__(self) -> None: ...

    @staticmethod
    def from_clip_from_world(clip_from_world: Mat4) -> ViewFrustum:
        """Creates a view frustum from a clip-from-world matrix."""

    @staticmethod
    def from_clip_from_world_custom_far(
        clip_from_world: Mat4,
        view_translation: Vec3,
        view_backward: Vec3,
        far: float,
    ) -> ViewFrustum:
        """Creates a view frustum from a clip-from-world matrix with a custom far plane."""

    def corners(self) -> list[Vec3] | None:
        """The 8 corner points of the frustum, or None if it is unbounded."""

    def __eq__(self, other: object) -> bool: ...

class Cylinder(Meshable):
    def __init__(self, radius: float = 0.5, height: float = 1.0) -> None: ...
    @property
    def radius(self) -> float: ...
    @radius.setter
    def radius(self, value: float) -> None: ...
    @property
    def half_height(self) -> float: ...
    @half_height.setter
    def half_height(self, value: float) -> None: ...
    def base(self) -> Circle: ...
    def base_area(self) -> float: ...
    def lateral_area(self) -> float: ...
    def area(self) -> float: ...
    def volume(self) -> float: ...
    def mesh(self) -> CylinderMeshBuilder: ...

class Cuboid(Meshable):
    def __init__(
        self,
        x_length: float = 1.0,
        y_length: float = 1.0,
        z_length: float = 1.0,
        *,
        half_size: Vec3 | None = None,
    ) -> None: ...
    @staticmethod
    def from_size(size: Vec3) -> Cuboid: ...
    @staticmethod
    def from_corners(point1: Vec3, point2: Vec3) -> Cuboid: ...
    @staticmethod
    def from_length(length: float) -> Cuboid: ...
    @property
    def half_size(self) -> Vec3: ...
    @half_size.setter
    def half_size(self, value: Vec3) -> None: ...
    def size(self) -> Vec3: ...
    def closest_point(self, point: Vec3) -> Vec3: ...
    def area(self) -> float: ...
    def volume(self) -> float: ...
    def mesh(self) -> CuboidMeshBuilder: ...

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

    def __init__(self, x: float, y: float, z: float, w: float) -> None: ...
    @staticmethod
    def splat(value: float) -> Vec4: ...
    def dot(self, other: Vec4) -> float: ...
    def length(self) -> float: ...
    def length_squared(self) -> float: ...
    def normalize(self) -> Vec4: ...
    def is_normalized(self) -> bool: ...
    def floor(self) -> Vec4: ...
    def ceil(self) -> Vec4: ...
    def min_element(self) -> float: ...
    def max_element(self) -> float: ...
    def truncate(self) -> Vec3: ...
    def xyz(self) -> Vec3: ...
    def xyzw(self) -> tuple[float, float, float, float]: ...
    @staticmethod
    def from_array(a: tuple[float, float, float, float]) -> Vec4: ...
    def to_array(self) -> tuple[float, float, float, float]: ...
    def as_tuple(self) -> tuple[float, float, float, float]: ...
    def abs(self) -> Vec4: ...
    def signum(self) -> Vec4: ...
    def copysign(self, rhs: Vec4) -> Vec4: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...
    def round(self) -> Vec4: ...
    def trunc(self) -> Vec4: ...
    def fract(self) -> Vec4: ...
    def fract_gl(self) -> Vec4: ...
    def exp(self) -> Vec4: ...
    def powf(self, n: float) -> Vec4: ...
    def recip(self) -> Vec4: ...
    def lerp(self, rhs: Vec4, s: float) -> Vec4: ...
    def move_towards(self, rhs: Vec4, d: float) -> Vec4: ...
    def midpoint(self, rhs: Vec4) -> Vec4: ...
    def project_onto(self, rhs: Vec4) -> Vec4: ...
    def reject_from(self, rhs: Vec4) -> Vec4: ...
    def normalize_or_zero(self) -> Vec4: ...
    def try_normalize(self) -> Vec4 | None: ...
    def distance(self, rhs: Vec4) -> float: ...
    def distance_squared(self, rhs: Vec4) -> float: ...
    def min(self, rhs: Vec4) -> Vec4: ...
    def max(self, rhs: Vec4) -> Vec4: ...
    def clamp(self, min: Vec4, max: Vec4) -> Vec4: ...
    def element_sum(self) -> float: ...
    def element_product(self) -> float: ...
    def __add__(self, other: float | Vec4) -> Vec4: ...
    def __sub__(self, other: float | Vec4) -> Vec4: ...
    def __mul__(self, other: float | Vec4) -> Vec4: ...
    def __div__(self, other: float | Vec4) -> Vec4: ...
    def __truediv__(self, other: float | Vec4) -> Vec4: ...
    def __neg__(self) -> Vec4: ...
    def __rmul__(self, other: float | Vec4) -> Vec4: ...

class Mat3:
    """A 3x3 matrix class."""
    IDENTITY: ClassVar[Mat3]
    ZERO: ClassVar[Mat3]
    NAN: ClassVar[Mat3]

    def __init__(
        self,
        m00: float, m01: float, m02: float,
        m10: float, m11: float, m12: float,
        m20: float, m21: float, m22: float,
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

    def col(self, index: int) -> Vec3: ...
    def row(self, index: int) -> Vec3: ...
    def to_cols_array(self) -> list[float]: ...
    def to_cols_array_2d(self) -> list[list[float]]: ...
    def transpose(self) -> Mat3: ...
    def determinant(self) -> float: ...
    def inverse(self) -> Mat3: ...
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

class Mat3A:
    """A 3x3 column-major matrix using SIMD-aligned Vec3A columns.

    Provides optimized 3x3 matrix operations with SIMD alignment.
    """

    IDENTITY: ClassVar[Mat3A]
    ZERO: ClassVar[Mat3A]
    NAN: ClassVar[Mat3A]

    def __init__(
        self,
        m00: float, m01: float, m02: float,
        m10: float, m11: float, m12: float,
        m20: float, m21: float, m22: float,
    ) -> None: ...

    @staticmethod
    def from_cols(x_axis: Vec3A, y_axis: Vec3A, z_axis: Vec3A) -> Mat3A: ...
    @staticmethod
    def from_cols_array(m: list[float]) -> Mat3A: ...
    @staticmethod
    def from_diagonal(diagonal: Vec3A) -> Mat3A: ...
    @staticmethod
    def from_rotation_x(angle: float) -> Mat3A: ...
    @staticmethod
    def from_rotation_y(angle: float) -> Mat3A: ...
    @staticmethod
    def from_rotation_z(angle: float) -> Mat3A: ...

    @property
    def x_axis(self) -> Vec3A: ...
    @property
    def y_axis(self) -> Vec3A: ...
    @property
    def z_axis(self) -> Vec3A: ...

    def col(self, index: int) -> Vec3A: ...
    def transpose(self) -> Mat3A: ...
    def determinant(self) -> float: ...
    def inverse(self) -> Mat3A: ...
    def mul_vec3a(self, rhs: Vec3A) -> Vec3A: ...
    def mul_mat3a(self, rhs: Mat3A) -> Mat3A: ...
    def abs(self) -> Mat3A: ...
    def is_finite(self) -> bool: ...
    def is_nan(self) -> bool: ...

    @overload
    def __mul__(self, other: float) -> Mat3A: ...
    @overload
    def __mul__(self, other: Mat3A) -> Mat3A: ...
    @overload
    def __mul__(self, other: Vec3A) -> Vec3A: ...
    def __rmul__(self, scalar: float) -> Mat3A: ...
    def __neg__(self) -> Mat3A: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class Mat4:
    """A 4x4 matrix class for 3D transformations."""

    IDENTITY: ClassVar[Mat4]
    ZERO: ClassVar[Mat4]
    NAN: ClassVar[Mat4]

    def __init__(
        self,
        m00: float, m01: float, m02: float, m03: float,
        m10: float, m11: float, m12: float, m13: float,
        m20: float, m21: float, m22: float, m23: float,
        m30: float, m31: float, m32: float, m33: float,
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
    def perspective_lh(fov_y_radians: float, aspect_ratio: float, z_near: float, z_far: float) -> Mat4: ...
    @staticmethod
    def perspective_rh(fov_y_radians: float, aspect_ratio: float, z_near: float, z_far: float) -> Mat4: ...
    @staticmethod
    def orthographic_lh(left: float, right: float, bottom: float, top: float, near: float, far: float) -> Mat4: ...
    @staticmethod
    def orthographic_rh(left: float, right: float, bottom: float, top: float, near: float, far: float) -> Mat4: ...
    @staticmethod
    def look_at_lh(eye: Vec3, center: Vec3, up: Vec3) -> Mat4: ...
    @staticmethod
    def look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Mat4: ...
    @staticmethod
    def look_to_lh(eye: Vec3, dir: Vec3, up: Vec3) -> Mat4: ...
    @staticmethod
    def look_to_rh(eye: Vec3, dir: Vec3, up: Vec3) -> Mat4: ...

    def col(self, index: int) -> Vec4: ...
    def row(self, index: int) -> Vec4: ...
    def to_cols_array(self) -> list[float]: ...
    def to_cols_array_2d(self) -> list[list[float]]: ...
    def transpose(self) -> Mat4: ...
    def determinant(self) -> float: ...
    def inverse(self) -> Mat4: ...
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
    def __len__(self) -> int: ...
    def __getitem__(self, index: int) -> list[float]: ...
    def __truediv__(self, scalar: float) -> Mat4: ...
    def __neg__(self) -> Mat4: ...

class Isometry2d:
    """2D isometry representing translation and rotation (rigid transform)."""

    IDENTITY: ClassVar[Isometry2d]

    @property
    def translation(self) -> Vec2: ...
    @property
    def rotation(self) -> Rot2: ...

    def __init__(self, translation: Vec2 = ..., rotation: Rot2 | None = None) -> None:
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

    def __eq__(self, other: object) -> bool: ...

class Aabb2d:
    """Axis-aligned bounding box in 2D space.

    An Aabb2d is defined by its minimum and maximum corners, and provides
    efficient collision detection and spatial queries.
    """

    min: Vec2
    max: Vec2

    def __init__(self, center: Vec2, half_size: Vec2) -> None:
        """Create a new Aabb2d from center and half-extents.

        Args:
            center: The center point of the bounding box
            half_size: Half the size of the bounding box in each dimension

        Example:
            ```python
            from pybevy.math import Aabb2d, Vec2

            aabb = Aabb2d(Vec2(50.0, 50.0), Vec2(50.0, 50.0))
            ```
        """

    @staticmethod
    def new(center: Vec2, half_size: Vec2) -> Aabb2d:
        """Create a new Aabb2d from center and half-extents."""

    @staticmethod
    def from_point_cloud(isometry: Isometry2d, points: list[Vec2]) -> Aabb2d:
        """Create an Aabb2d that contains all given points.

        Args:
            isometry: Transform to apply to points
            points: List of Vec2 points to bound

        Returns:
            The smallest Aabb2d containing all transformed points
        """

    def center(self) -> Vec2:
        """Get the center point of the bounding box."""

    def half_size(self) -> Vec2:
        """Get the half-extents of the bounding box."""

    def closest_point(self, point: Vec2) -> Vec2:
        """Find the closest point on the bounding box to the given point."""

    def contains(self, other: Aabb2d) -> bool:
        """Check if this bounding box completely contains another."""

    def merge(self, other: Aabb2d) -> Aabb2d:
        """Merge with another bounding box, returning the smallest box containing both."""

    def grow(self, amount: Vec2) -> Aabb2d:
        """Grow the bounding box by the given amount in all directions."""

    def shrink(self, amount: Vec2) -> Aabb2d:
        """Shrink the bounding box by the given amount in all directions."""

    def scale_around_center(self, scale: Vec2) -> Aabb2d:
        """Scale the bounding box around its center."""

    def visible_area(self) -> float:
        """Calculate the perimeter of the bounding box."""

    def bounding_circle(self) -> BoundingCircle:
        """Get the bounding circle that contains this bounding box."""

    def intersects_aabb(self, other: Aabb2d) -> bool:
        """Check if this bounding box intersects with another Aabb2d."""

    def intersects_circle(self, circle: BoundingCircle) -> bool:
        """Check if this bounding box intersects with a BoundingCircle."""

class BoundingCircle:
    """Bounding circle in 2D space.

    A BoundingCircle is defined by its center point and radius, providing
    efficient circular collision detection.
    """

    center: Vec2

    def __init__(self, center: Vec2, radius: float) -> None:
        """Create a new BoundingCircle.

        Args:
            center: The center point of the circle
            radius: The radius of the circle

        Example:
            ```python
            from pybevy.math import BoundingCircle, Vec2

            # Create a circle with radius 50 centered at (100, 100)
            circle = BoundingCircle(Vec2(100.0, 100.0), 50.0)
            ```
        """

    @staticmethod
    def new(center: Vec2, radius: float) -> BoundingCircle:
        """Create a new BoundingCircle."""

    @staticmethod
    def from_point_cloud(isometry: Isometry2d, points: list[Vec2]) -> BoundingCircle:
        """Create a BoundingCircle that contains all given points.

        Args:
            isometry: Transform to apply to points
            points: List of Vec2 points to bound

        Returns:
            The smallest BoundingCircle containing all transformed points
        """

    def radius(self) -> float:
        """Get the radius of the circle."""

    def closest_point(self, point: Vec2) -> Vec2:
        """Find the closest point on the circle to the given point."""

    def contains(self, other: BoundingCircle) -> bool:
        """Check if this circle completely contains another circle."""

    def merge(self, other: BoundingCircle) -> BoundingCircle:
        """Merge with another circle, returning the smallest circle containing both."""

    def grow(self, amount: float) -> BoundingCircle:
        """Grow the circle radius by the given amount."""

    def shrink(self, amount: float) -> BoundingCircle:
        """Shrink the circle radius by the given amount."""

    def scale_around_center(self, scale: float) -> BoundingCircle:
        """Scale the circle around its center."""

    def visible_area(self) -> float:
        """Calculate the circumference of the circle."""

    def aabb_2d(self) -> Aabb2d:
        """Get the axis-aligned bounding box that contains this circle."""

    def intersects_circle(self, other: BoundingCircle) -> bool:
        """Check if this circle intersects with another BoundingCircle."""

    def intersects_aabb(self, aabb: Aabb2d) -> bool:
        """Check if this circle intersects with an Aabb2d."""

class Isometry3d:
    """3D isometry (rotation + translation) for bounding volume transformations.

    An isometry represents a rigid transformation that preserves distances -
    a combination of rotation and translation, without any scaling.
    """

    IDENTITY: ClassVar[Isometry3d]

    translation: Vec3
    rotation: Quat

    def __init__(self, translation: Vec3 = ..., rotation: Quat = ...) -> None:
        """Create a new Isometry3d.

        Args:
            translation: The translation component (default: Vec3.ZERO)
            rotation: The rotation component (default: Quat.IDENTITY)
        """

    @staticmethod
    def from_xyz(x: float, y: float, z: float) -> Isometry3d:
        """Create an isometry from translation coordinates with identity rotation."""

    @staticmethod
    def from_translation(translation: Vec3) -> Isometry3d:
        """Create an isometry from a translation vector with identity rotation."""

    @staticmethod
    def from_rotation(rotation: Quat) -> Isometry3d:
        """Create an isometry from a rotation with zero translation."""

    def inverse(self) -> Isometry3d:
        """Get the inverse of this isometry."""

    def inverse_mul(self, rhs: Isometry3d) -> Isometry3d:
        """Compute the equivalent of applying the inverse of self followed by rhs."""

    def transform_point(self, point: Vec3) -> Vec3:
        """Transform a point by this isometry (rotate then translate)."""

    def inverse_transform_point(self, point: Vec3) -> Vec3:
        """Inverse-transform a point (undo translation then undo rotation)."""

class Aabb3d:
    """Axis-aligned bounding box in 3D space."""

    min: Vec3
    max: Vec3

    def __init__(self, center: Vec3, half_size: Vec3) -> None:
        """Create a new Aabb3d from a center point and half-extents.

        Args:
            center: The center point of the box
            half_size: The half-extents (half the width, height, depth)

        Example:
            >>> from pybevy.math import Aabb3d, Vec3
            >>> aabb = Aabb3d(Vec3(100.0, 100.0, 100.0), Vec3(25.0, 25.0, 25.0))
        """

    @staticmethod
    def from_min_max(min: Vec3, max: Vec3) -> Aabb3d:
        """Create an Aabb3d directly from minimum and maximum corners.

        Args:
            min: The minimum corner of the bounding box
            max: The maximum corner of the bounding box

        Example:
            >>> from pybevy.math import Aabb3d, Vec3
            >>> aabb = Aabb3d.from_min_max(Vec3(0.0, 0.0, 0.0), Vec3(100.0, 100.0, 100.0))
        """

    def center(self) -> Vec3:
        """Center of the bounding box."""

    def half_size(self) -> Vec3:
        """Half-extents of the bounding box."""

    def closest_point(self, point: Vec3) -> Vec3:
        """Find the closest point on the AABB to the given point."""

    def contains(self, other: Aabb3d) -> bool:
        """Check if this AABB contains another AABB."""

    def merge(self, other: Aabb3d) -> Aabb3d:
        """Merge this AABB with another, returning the smallest AABB containing both."""

    def grow(self, amount: Vec3) -> Aabb3d:
        """Grow the AABB by the given amount in each direction."""

    def shrink(self, amount: Vec3) -> Aabb3d:
        """Shrink the AABB by the given amount in each direction."""

    def scale_around_center(self, scale: Vec3) -> Aabb3d:
        """Scale the AABB around its center."""

    def visible_area(self) -> float:
        """Calculate the surface area of the AABB."""

    def bounding_sphere(self) -> BoundingSphere:
        """Get the bounding sphere that contains this AABB."""

    def intersects_aabb(self, other: Aabb3d) -> bool:
        """Check if this AABB intersects with another Aabb3d."""

    def intersects_sphere(self, sphere: BoundingSphere) -> bool:
        """Check if this AABB intersects with a BoundingSphere."""

    @staticmethod
    def from_point_cloud(isometry: Isometry3d, points: list[Vec3]) -> Aabb3d:
        """Create an Aabb3d from a collection of points.

        Args:
            isometry: Transformation to apply to the points
            points: List of Vec3 points to create the bounding box from

        Example:
            >>> from pybevy.math import Aabb3d, Vec3, Isometry3d
            >>> points = [Vec3(0.0, 0.0, 0.0), Vec3(10.0, 10.0, 10.0)]
            >>> aabb = Aabb3d.from_point_cloud(Isometry3d.IDENTITY, points)
        """

class BoundingSphere:
    """Bounding sphere in 3D space."""

    center: Vec3

    def __init__(self, center: Vec3, radius: float) -> None:
        """Create a new BoundingSphere from a center point and radius.

        Args:
            center: The center point of the sphere
            radius: The radius of the sphere

        Example:
            >>> from pybevy.math import BoundingSphere, Vec3
            >>> sphere = BoundingSphere(Vec3(100.0, 100.0, 100.0), 50.0)
        """

    def radius(self) -> float:
        """Radius of the bounding sphere."""

    def closest_point(self, point: Vec3) -> Vec3:
        """Find the closest point on the sphere to the given point."""

    def contains(self, other: BoundingSphere) -> bool:
        """Check if this sphere contains another sphere."""

    def merge(self, other: BoundingSphere) -> BoundingSphere:
        """Merge this sphere with another, returning the smallest sphere containing both."""

    def grow(self, amount: float) -> BoundingSphere:
        """Grow the sphere by the given amount."""

    def shrink(self, amount: float) -> BoundingSphere:
        """Shrink the sphere by the given amount."""

    def scale_around_center(self, scale: float) -> BoundingSphere:
        """Scale the sphere around its center."""

    def visible_area(self) -> float:
        """Calculate the surface area of the sphere."""

    def aabb_3d(self) -> Aabb3d:
        """Get the axis-aligned bounding box that contains this sphere."""

    def intersects_sphere(self, other: BoundingSphere) -> bool:
        """Check if this sphere intersects with another BoundingSphere."""

    def intersects_aabb(self, aabb: Aabb3d) -> bool:
        """Check if this sphere intersects with an Aabb3d."""

    @staticmethod
    def from_point_cloud(isometry: Isometry3d, points: list[Vec3]) -> BoundingSphere:
        """Create a BoundingSphere from a collection of points.

        Args:
            isometry: Transformation to apply to the points
            points: List of Vec3 points to create the bounding sphere from

        Example:
            >>> from pybevy.math import BoundingSphere, Vec3, Isometry3d
            >>> points = [Vec3(0.0, 0.0, 0.0), Vec3(10.0, 10.0, 10.0)]
            >>> sphere = BoundingSphere.from_point_cloud(Isometry3d.IDENTITY, points)
        """

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

    x: float
    y: float

    def __init__(self, x: float, y: float) -> None:
        """Create a normalized direction from x, y components.

        Raises:
            ValueError: If the vector has zero length
        """

    @staticmethod
    def from_vec2(vec: Vec2) -> Dir2:
        """Create a normalized direction from a Vec2.

        Raises:
            ValueError: If the vector has zero length
        """

    def as_vec2(self) -> Vec2:
        """Convert to Vec2."""

    def dot(self, other: Dir2) -> float:
        """Compute dot product with another direction."""

    def perp(self) -> Dir2:
        """Get perpendicular direction (rotated 90 degrees counterclockwise)."""

    def slerp(self, rhs: Dir2, s: float) -> Dir2:
        """Spherical linear interpolation between two directions."""

    def as_tuple(self) -> tuple[float, float]:
        """Convert to tuple."""

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

class Ray2d:
    """A 2D ray defined by an origin point and direction."""

    origin: Vec2
    direction: Dir2

    def __init__(self, origin: Vec2, direction: Dir2) -> None:
        """Create a new Ray2d.

        Args:
            origin: Starting point of the ray
            direction: Normalized direction vector

        Example:
            >>> from pybevy.math import Ray2d, Vec2, Dir2
            >>> ray = Ray2d(Vec2(0.0, 0.0), Dir2.X)
        """

    def get_point(self, distance: float) -> Vec2:
        """Get a point along the ray at the given distance.

        Args:
            distance: Distance from the origin

        Example:
            >>> ray = Ray2d(Vec2(0.0, 0.0), Dir2.X)
            >>> point = ray.get_point(5.0)  # Vec2(5.0, 0.0)
        """

    def intersect_plane(self, plane_origin: Vec2, plane: Plane2d) -> float | None:
        """Get the distance to a plane if the ray intersects it.

        Args:
            plane_origin: A point on the plane
            plane: The plane to test intersection against

        Returns:
            The distance to the intersection point, or None if no intersection.
        """

    def plane_intersection_point(self, plane_origin: Vec2, plane: Plane2d) -> Vec2 | None:
        """Get the intersection point with a plane, or None if no intersection.

        Args:
            plane_origin: A point on the plane
            plane: The plane to test intersection against

        Returns:
            The intersection point, or None if no intersection.
        """

    def __eq__(self, other: object) -> bool: ...

class Ray3d:
    """A 3D ray defined by an origin point and direction."""

    origin: Vec3
    direction: Dir3

    def __init__(self, origin: Vec3, direction: Dir3) -> None:
        """Create a new Ray3d.

        Args:
            origin: Starting point of the ray
            direction: Normalized direction vector

        Example:
            >>> from pybevy.math import Ray3d, Vec3, Dir3
            >>> ray = Ray3d(Vec3(0.0, 0.0, 0.0), Dir3.X)
        """

    def get_point(self, distance: float) -> Vec3:
        """Get a point along the ray at the given distance.

        Args:
            distance: Distance from the origin

        Example:
            >>> ray = Ray3d(Vec3(0.0, 0.0, 0.0), Dir3.X)
            >>> point = ray.get_point(5.0)  # Vec3(5.0, 0.0, 0.0)
        """

    def intersect_plane(
        self, plane_origin: Vec3, plane: InfinitePlane3d
    ) -> float | None:
        """Get the distance to a plane if the ray intersects it.

        Args:
            plane_origin: A point on the plane
            plane: The infinite plane to test intersection against

        Returns:
            The distance to the intersection point, or None if no intersection.
        """

    def plane_intersection_point(
        self, plane_origin: Vec3, plane: InfinitePlane3d
    ) -> Vec3 | None:
        """Get the intersection point with a plane, or None if no intersection.

        Args:
            plane_origin: A point on the plane
            plane: The infinite plane to test intersection against

        Returns:
            The intersection point, or None if no intersection.
        """

    def __eq__(self, other: object) -> bool: ...

class RayCast2d:
    """A 2D raycast with a maximum distance for intersection testing."""

    ray: Ray2d
    max: float

    def __init__(self, origin: Vec2, direction: Dir2, max: float) -> None:
        """Create a new RayCast2d.

        Args:
            origin: Starting point of the ray
            direction: Normalized direction vector
            max: Maximum distance for the raycast

        Example:
            >>> from pybevy.math import RayCast2d, Vec2, Dir2
            >>> raycast = RayCast2d(Vec2(0.0, 0.0), Dir2.X, 100.0)
        """

    @staticmethod
    def from_ray(ray: Ray2d, max: float) -> RayCast2d:
        """Create a RayCast2d from a Ray2d and maximum distance."""

    def intersects_aabb(self, aabb: Aabb2d) -> bool:
        """Check if the ray intersects with an Aabb2d within max distance."""

    def intersects_circle(self, circle: BoundingCircle) -> bool:
        """Check if the ray intersects with a BoundingCircle within max distance."""

    def aabb_intersection_at(self, aabb: Aabb2d) -> float | None:
        """Get the distance to intersection with Aabb2d, or None."""

    def circle_intersection_at(self, circle: BoundingCircle) -> float | None:
        """Get the distance to intersection with BoundingCircle, or None."""

    def direction_recip(self) -> Vec2:
        """Get the reciprocal of the ray direction (1.0 / direction for each component)."""

class RayCast3d:
    """A 3D raycast with a maximum distance for intersection testing."""

    ray: Ray3d
    max: float

    def __init__(self, origin: Vec3, direction: Dir3, max: float) -> None:
        """Create a new RayCast3d.

        Args:
            origin: Starting point of the ray
            direction: Normalized direction vector
            max: Maximum distance for the raycast

        Example:
            >>> from pybevy.math import RayCast3d, Vec3, Dir3
            >>> raycast = RayCast3d(Vec3(0.0, 0.0, 0.0), Dir3.X, 100.0)
        """

    @staticmethod
    def from_ray(ray: Ray3d, max: float) -> RayCast3d:
        """Create a RayCast3d from a Ray3d and maximum distance."""

    def intersects_aabb(self, aabb: Aabb3d) -> bool:
        """Check if the ray intersects with an Aabb3d within max distance."""

    def intersects_sphere(self, sphere: BoundingSphere) -> bool:
        """Check if the ray intersects with a BoundingSphere within max distance."""

    def aabb_intersection_at(self, aabb: Aabb3d) -> float | None:
        """Get the distance to intersection with Aabb3d, or None."""

    def sphere_intersection_at(self, sphere: BoundingSphere) -> float | None:
        """Get the distance to intersection with BoundingSphere, or None."""

    def direction_recip(self) -> Vec3:
        """Get the reciprocal of the ray direction (1.0 / direction for each component)."""

class JumpAt:
    """Specifies where a jump should occur in a stepped easing function."""

    Start: ClassVar[JumpAt]
    """Jump occurs at the start"""
    End: ClassVar[JumpAt]
    """Jump occurs at the end"""
    None_: ClassVar[JumpAt]
    """No jump"""
    Both: ClassVar[JumpAt]
    """Jump occurs at both start and end"""

    def __eq__(self, other: object) -> bool: ...

class EaseFunction:
    """Easing functions for smooth interpolation and animation.
    
    These functions map a linear input (0.0 to 1.0) to an eased output,
    providing various animation curves like ease-in, ease-out, and ease-in-out.
    
    Example:
        >>> from pybevy.math import EaseFunction
        >>> # Linear interpolation (no easing)
        >>> t = 0.5
        >>> eased = EaseFunction.Linear.ease(t)  # Returns 0.5
        >>> 
        >>> # Ease in with quadratic curve (slow start, fast end)
        >>> eased = EaseFunction.QuadraticIn.ease(0.5)  # Returns 0.25
        >>> 
        >>> # Ease in-out with cubic curve (slow start and end)
        >>> eased = EaseFunction.CubicInOut.ease(0.5)  # Returns 0.5
        >>> 
        >>> # Elastic easing with custom amplitude
        >>> eased = EaseFunction.Elastic(2.0).ease(0.8)
        >>> 
        >>> # Stepped easing
        >>> from pybevy.math import JumpAt
        >>> eased = EaseFunction.Steps(5, JumpAt.End).ease(0.6)
    """
    
    Linear: ClassVar[EaseFunction]
    """Linear interpolation (no easing)"""
    QuadraticIn: ClassVar[EaseFunction]
    """Quadratic ease-in (slow start)"""
    QuadraticOut: ClassVar[EaseFunction]
    """Quadratic ease-out (slow end)"""
    QuadraticInOut: ClassVar[EaseFunction]
    """Quadratic ease-in-out (slow start and end)"""
    CubicIn: ClassVar[EaseFunction]
    """Cubic ease-in"""
    CubicOut: ClassVar[EaseFunction]
    """Cubic ease-out"""
    CubicInOut: ClassVar[EaseFunction]
    """Cubic ease-in-out"""
    QuarticIn: ClassVar[EaseFunction]
    """Quartic ease-in"""
    QuarticOut: ClassVar[EaseFunction]
    """Quartic ease-out"""
    QuarticInOut: ClassVar[EaseFunction]
    """Quartic ease-in-out"""
    QuinticIn: ClassVar[EaseFunction]
    """Quintic ease-in"""
    QuinticOut: ClassVar[EaseFunction]
    """Quintic ease-out"""
    QuinticInOut: ClassVar[EaseFunction]
    """Quintic ease-in-out"""
    SineIn: ClassVar[EaseFunction]
    """Sine ease-in"""
    SineOut: ClassVar[EaseFunction]
    """Sine ease-out"""
    SineInOut: ClassVar[EaseFunction]
    """Sine ease-in-out"""
    CircularIn: ClassVar[EaseFunction]
    """Circular ease-in"""
    CircularOut: ClassVar[EaseFunction]
    """Circular ease-out"""
    CircularInOut: ClassVar[EaseFunction]
    """Circular ease-in-out"""
    ExponentialIn: ClassVar[EaseFunction]
    """Exponential ease-in"""
    ExponentialOut: ClassVar[EaseFunction]
    """Exponential ease-out"""
    ExponentialInOut: ClassVar[EaseFunction]
    """Exponential ease-in-out"""
    ElasticIn: ClassVar[EaseFunction]
    """Elastic ease-in with default amplitude"""
    ElasticOut: ClassVar[EaseFunction]
    """Elastic ease-out with default amplitude"""
    ElasticInOut: ClassVar[EaseFunction]
    """Elastic ease-in-out with default amplitude"""
    BackIn: ClassVar[EaseFunction]
    """Back ease-in (overshoots then returns)"""
    BackOut: ClassVar[EaseFunction]
    """Back ease-out"""
    BackInOut: ClassVar[EaseFunction]
    """Back ease-in-out"""
    BounceIn: ClassVar[EaseFunction]
    """Bounce ease-in"""
    BounceOut: ClassVar[EaseFunction]
    """Bounce ease-out"""
    BounceInOut: ClassVar[EaseFunction]
    """Bounce ease-in-out"""
    SmoothStep: ClassVar[EaseFunction]
    """Smooth step function"""
    SmoothStepIn: ClassVar[EaseFunction]
    """Smooth step ease-in"""
    SmoothStepOut: ClassVar[EaseFunction]
    """Smooth step ease-out"""
    SmootherStep: ClassVar[EaseFunction]
    """Smoother step function (5th order)"""
    SmootherStepIn: ClassVar[EaseFunction]
    """Smoother step ease-in"""
    SmootherStepOut: ClassVar[EaseFunction]
    """Smoother step ease-out"""
    
    @staticmethod
    def Elastic(amplitude: float) -> EaseFunction:
        """Create an elastic easing function with custom amplitude.
        
        Args:
            amplitude: The amplitude of the elastic effect
            
        Returns:
            An elastic easing function
        """
    
    @staticmethod
    def Steps(steps: int, jump: JumpAt) -> EaseFunction:
        """Create a stepped easing function.
        
        Args:
            steps: Number of steps
            jump: Where jumps should occur (Start, End, None, or Both)
            
        Returns:
            A stepped easing function
        """
    
    def ease(self, t: float) -> float:
        """Evaluate the easing function at time t (typically 0.0 to 1.0).
        
        Args:
            t: Time parameter, typically in range [0.0, 1.0]
            
        Returns:
            Eased value
            
        Example:
            >>> from pybevy.math import EaseFunction
            >>> # Linear: input = output
            >>> assert EaseFunction.Linear.ease(0.5) == 0.5
            >>> # QuadraticIn: t^2
            >>> assert abs(EaseFunction.QuadraticIn.ease(0.5) - 0.25) < 0.001
            >>> # Animate a value
            >>> start = 0.0
            >>> end = 100.0
            >>> progress = 0.3  # 30% through animation
            >>> eased_progress = EaseFunction.CubicInOut.ease(progress)
            >>> current_value = start + (end - start) * eased_progress
        """

class Capsule2d(Meshable):
    """A 2D capsule primitive (pill shape).

    A capsule is a shape consisting of a rectangle with semicircular ends.
    It's defined by a radius and a length (distance between the centers of the semicircles).

    Example:
        >>> from pybevy.math import Capsule2d
        >>> # Create a capsule with radius 5.0 and length 20.0
        >>> capsule = Capsule2d(5.0, 20.0)
        >>> mesh = capsule.mesh().build()  # Create mesh from primitive
    """

    @property
    def radius(self) -> float: ...
    @radius.setter
    def radius(self, value: float) -> None: ...
    @property
    def half_length(self) -> float: ...
    @half_length.setter
    def half_length(self, value: float) -> None: ...

    def __init__(self, radius: float = 0.5, length: float = 1.0) -> None:
        """Create a new 2D capsule.

        Args:
            radius: The radius of the semicircular ends
            length: The distance between the centers of the semicircles
        """

    def inner_rectangle(self) -> Rectangle:
        """Get the inner rectangle of the capsule (excluding the semicircular ends)."""

    def to_inner_rectangle(self) -> Rectangle:
        """Get the inner rectangle of the capsule (excluding the semicircular ends)."""

    def area(self) -> float:
        """Get the area of the capsule."""

    def perimeter(self) -> float:
        """Get the perimeter of the capsule."""

    def mesh(self) -> Capsule2dMeshBuilder:
        """Create a mesh builder for this capsule."""

class Ellipse(Meshable):
    """A 2D ellipse primitive.

    An ellipse is defined by its half-size (semi-major and semi-minor axes).

    Example:
        >>> from pybevy.math import Ellipse, Vec2
        >>> # Create an ellipse with semi-axes of 3.0 and 2.0
        >>> ellipse = Ellipse(Vec2(3.0, 2.0))
        >>> mesh = ellipse.mesh().build()  # Create mesh from primitive
    """

    @property
    def half_size(self) -> Vec2: ...
    @half_size.setter
    def half_size(self, value: Vec2) -> None: ...

    def __init__(self, half_size: Vec2 = ...) -> None:
        """Create a new ellipse from its size.

        Args:
            half_size: A Vec2 containing the semi-major and semi-minor axes
        """

    def eccentricity(self) -> float:
        """Calculate the eccentricity of the ellipse.

        Eccentricity measures how much the ellipse deviates from being circular:
        - e = 0: perfect circle
        - 0 < e < 1: ellipse
        """

    def focal_length(self) -> float:
        """Calculate the focal length of the ellipse.

        The focal length is the distance from the center to each focus point.
        """

    def semi_major(self) -> float:
        """Get the length of the semi-major axis (longest radius)."""

    def semi_minor(self) -> float:
        """Get the length of the semi-minor axis (shortest radius)."""

    @staticmethod
    def from_size(size: Vec2) -> Ellipse:
        """Create a new ellipse from a given full size."""

    def area(self) -> float:
        """Get the area of the ellipse."""

    def perimeter(self) -> float:
        """Get an approximation of the perimeter of the ellipse."""

    def mesh(self) -> EllipseMeshBuilder:
        """Create a mesh builder for this ellipse."""

class WindingOrder:
    """Winding order of a triangle.

    Indicates whether the vertices of a triangle are ordered clockwise,
    counter-clockwise, or are collinear (invalid winding).
    """

    Clockwise: WindingOrder
    CounterClockwise: WindingOrder
    Invalid: WindingOrder

class Triangle2d(Meshable):
    """A 2D triangle primitive.

    A triangle is defined by its three vertices.

    Example:
        >>> from pybevy.math import Triangle2d, Vec2
        >>> tri = Triangle2d(
        ...     Vec2(0.0, 0.0),
        ...     Vec2(1.0, 0.0),
        ...     Vec2(0.5, 1.0)
        ... )
        >>> mesh = tri.mesh().build()  # Create mesh from primitive
    """

    @property
    def vertices(self) -> tuple[Vec2, Vec2, Vec2]: ...
    @vertices.setter
    def vertices(self, value: tuple[Vec2, Vec2, Vec2] | list[Vec2]) -> None: ...

    def __init__(
        self,
        a: Vec2 | None = None,
        b: Vec2 | None = None,
        c: Vec2 | None = None,
        *,
        vertices: list[Vec2] | None = None,
    ) -> None:
        """Create a new 2D triangle from three vertices.

        Args:
            a: First vertex
            b: Second vertex
            c: Third vertex
            vertices: Alternatively, provide vertices as a list of 3 Vec2
        """

    def is_acute(self) -> bool:
        """Check if the triangle is acute (all angles less than 90 degrees)."""

    def is_degenerate(self) -> bool:
        """Check if the triangle is degenerate (has zero area)."""

    def circumcircle(self) -> tuple[Circle, Vec2]:
        """Calculate the circumcircle of the triangle.

        Returns:
            A tuple of (Circle, center_position)
        """

    def area(self) -> float:
        """Get the area of the triangle."""

    def perimeter(self) -> float:
        """Get the perimeter of the triangle."""

    def winding_order(self) -> WindingOrder:
        """Get the winding order of the triangle vertices."""

    def is_obtuse(self) -> bool:
        """Check if the triangle is obtuse (has an angle greater than 90 degrees)."""

    def reverse(self) -> None:
        """Reverse the triangle's vertex order in place (flip winding)."""

    def reversed(self) -> Triangle2d:
        """Get a new triangle with reversed vertex order (opposite winding)."""

    def mesh(self) -> Triangle2dMeshBuilder:
        """Create a mesh builder for this triangle."""

class Capsule3d(Meshable):
    """A 3D capsule primitive (pill shape).

    A capsule is a cylinder with hemispherical ends. It's defined by a radius
    and a length (distance between the centers of the hemispheres).

    Example:
        >>> from pybevy.math import Capsule3d
        >>> # Create a capsule with radius 0.5 and length 2.0
        >>> capsule = Capsule3d(0.5, 2.0)
        >>> mesh = capsule.mesh().build()  # Create mesh from primitive
    """

    @property
    def radius(self) -> float: ...
    @radius.setter
    def radius(self, value: float) -> None: ...
    @property
    def half_length(self) -> float: ...
    @half_length.setter
    def half_length(self, value: float) -> None: ...

    def __init__(self, radius: float, length: float) -> None:
        """Create a new 3D capsule.

        Args:
            radius: The radius of the hemispherical ends and cylindrical body
            length: The distance between the centers of the hemispheres
        """

    def to_cylinder(self) -> Cylinder:
        """Get the inner cylinder of the capsule (excluding the hemispherical ends)."""

    def area(self) -> float:
        """Get the surface area of the capsule."""

    def volume(self) -> float:
        """Get the volume of the capsule."""

    def mesh(self) -> Capsule3dMeshBuilder:
        """Create a mesh builder for this capsule."""

class Triangle3d(Meshable):
    """A 3D triangle primitive.

    A triangle in 3D space is defined by its three vertices.

    Example:
        >>> from pybevy.math import Triangle3d, Vec3
        >>> tri = Triangle3d(
        ...     Vec3(0.0, 0.0, 0.0),
        ...     Vec3(1.0, 0.0, 0.0),
        ...     Vec3(0.5, 1.0, 0.0)
        ... )
        >>> mesh = tri.mesh().build()  # Create mesh from primitive
    """

    @property
    def vertices(self) -> tuple[Vec3, Vec3, Vec3]: ...
    @vertices.setter
    def vertices(self, value: tuple[Vec3, Vec3, Vec3] | list[Vec3]) -> None: ...

    def __init__(
        self,
        a: Vec3 | None = None,
        b: Vec3 | None = None,
        c: Vec3 | None = None,
        *,
        vertices: list[Vec3] | None = None,
    ) -> None:
        """Create a new 3D triangle from three vertices.

        Args:
            a: First vertex
            b: Second vertex
            c: Third vertex
            vertices: Alternatively, provide vertices as a list of 3 Vec3
        """

    def is_acute(self) -> bool:
        """Check if the triangle is acute (all angles less than 90 degrees)."""

    def centroid(self) -> Vec3:
        """Calculate the centroid (center of mass) of the triangle."""

    def circumcenter(self) -> Vec3:
        """Calculate the circumcenter of the triangle.

        The circumcenter is the center of the circle that passes through all three vertices.
        """

    def area(self) -> float:
        """Get the area of the triangle."""

    def perimeter(self) -> float:
        """Get the perimeter of the triangle."""

    def normal(self) -> Dir3:
        """Get the surface normal of the triangle."""

    def is_degenerate(self) -> bool:
        """Check if the triangle is degenerate (has zero area)."""

    def is_obtuse(self) -> bool:
        """Check if the triangle is obtuse (has an angle greater than 90 degrees)."""

    def largest_side(self) -> float:
        """Get the length of the largest side of the triangle."""

    def reverse(self) -> None:
        """Reverse the triangle's vertex order in place (flip winding)."""

    def reversed(self) -> Triangle3d:
        """Get a new triangle with reversed vertex order (opposite winding)."""

    def mesh(self) -> Triangle3dMeshBuilder:
        """Create a mesh builder for this triangle."""

class RegularPolygon(Meshable):
    """A 2D regular polygon primitive.

    A regular polygon is a polygon with all sides and angles equal. It's defined by
    a circumradius (radius of the circle passing through all vertices) and the number of sides.

    Example:
        >>> from pybevy.math import RegularPolygon
        >>> # Create a hexagon with circumradius 1.0
        >>> hexagon = RegularPolygon(1.0, 6)
        >>> mesh = hexagon.mesh().build()  # Create mesh from primitive
    """

    @property
    def circumcircle(self) -> Circle: ...
    @property
    def sides(self) -> int: ...
    @sides.setter
    def sides(self, value: int) -> None: ...

    def __init__(self, circumradius: float = 0.5, sides: int = 6) -> None:
        """Create a new regular polygon.

        Args:
            circumradius: The radius of the circumcircle (circle passing through all vertices)
            sides: The number of sides (must be at least 3)
        """

    def circumradius(self) -> float:
        """Get the radius of the circumcircle."""

    def inradius(self) -> float:
        """Get the inradius (apothem) of the regular polygon.

        This is the radius of the largest circle that can be drawn within the polygon.
        """

    def side_length(self) -> float:
        """Get the length of one side of the regular polygon."""

    def internal_angle_degrees(self) -> float:
        """Get the internal angle of the regular polygon in degrees."""

    def internal_angle_radians(self) -> float:
        """Get the internal angle of the regular polygon in radians."""

    def external_angle_degrees(self) -> float:
        """Get the external angle of the regular polygon in degrees."""

    def external_angle_radians(self) -> float:
        """Get the external angle of the regular polygon in radians."""

    def area(self) -> float:
        """Get the area of the regular polygon."""

    def perimeter(self) -> float:
        """Get the perimeter of the regular polygon."""

    def mesh(self) -> RegularPolygonMeshBuilder:
        """Create a mesh builder for this regular polygon."""


class Rhombus(Meshable):
    """A 2D rhombus (diamond) shape defined by its half-diagonals.

    A rhombus is a quadrilateral with all sides equal in length.
    It can be thought of as a diamond shape.

    Example:
        >>> from pybevy.math import Rhombus
        >>> # Create a rhombus with horizontal diagonal 2.0 and vertical diagonal 3.0
        >>> rhombus = Rhombus(2.0, 3.0)
        >>> mesh = rhombus.mesh().build()  # Create mesh from primitive
    """

    @property
    def half_diagonals(self) -> Vec2: ...
    @half_diagonals.setter
    def half_diagonals(self, value: Vec2) -> None: ...

    def __init__(
        self,
        horizontal_diagonal: float = 1.0,
        vertical_diagonal: float = 1.0,
        *,
        half_diagonals: Vec2 | None = None,
    ) -> None:
        """Create a new Rhombus from horizontal and vertical diagonal sizes.

        Args:
            horizontal_diagonal: The full horizontal diagonal size
            vertical_diagonal: The full vertical diagonal size
            half_diagonals: Alternatively, provide half-diagonal Vec2 directly
        """

    @staticmethod
    def from_side(side: float) -> Rhombus:
        """Create a rhombus from a side length with all inner angles equal (a square rotated 45 degrees).

        Args:
            side: The length of each side

        Returns:
            A new Rhombus with the given side length
        """

    @staticmethod
    def from_inradius(inradius: float) -> Rhombus:
        """Create a rhombus from the inradius (radius of inscribed circle) with all inner angles equal.

        Args:
            inradius: The radius of the inscribed circle

        Returns:
            A new Rhombus with the given inradius
        """

    def side(self) -> float:
        """Get the length of each side of the rhombus."""

    def circumradius(self) -> float:
        """Get the circumradius (radius of circumscribed circle)."""

    def inradius(self) -> float:
        """Get the inradius (radius of inscribed circle)."""

    def area(self) -> float:
        """Get the area of the rhombus."""

    def perimeter(self) -> float:
        """Get the perimeter of the rhombus."""

    def closest_point(self, point: Vec2) -> Vec2:
        """Find the point on the rhombus that is closest to the given point."""

    def mesh(self) -> RhombusMeshBuilder:
        """Create a mesh builder for this rhombus."""


class Annulus(Meshable):
    """A 2D annulus (ring) primitive.

    An annulus is the region between two concentric circles. Also known as a ring.

    Example:
        >>> from pybevy.math import Annulus
        >>> # Create an annulus with inner radius 0.5 and outer radius 1.0
        >>> ring = Annulus(0.5, 1.0)
        >>> mesh = ring.mesh().build()  # Create mesh from primitive
    """

    inner_circle: Circle
    outer_circle: Circle

    def __init__(
        self,
        inner_radius: float = 0.5,
        outer_radius: float = 1.0,
        *,
        inner_circle: Circle | None = None,
        outer_circle: Circle | None = None,
    ) -> None:
        """Create a new annulus.

        Args:
            inner_radius: The radius of the inner circle (hole)
            outer_radius: The radius of the outer circle
            inner_circle: Alternatively, provide inner Circle directly
            outer_circle: Alternatively, provide outer Circle directly
        """

    def diameter(self) -> float:
        """Get the diameter of the annulus (outer diameter)."""

    def thickness(self) -> float:
        """Get the thickness of the annulus (difference between outer and inner radius)."""

    def area(self) -> float:
        """Get the area of the annulus."""

    def perimeter(self) -> float:
        """Get the perimeter of the annulus (sum of inner and outer circumferences)."""

    def closest_point(self, point: Vec2) -> Vec2:
        """Find the point on the annulus that is closest to the given point.

        Args:
            point: The point to find the closest point to

        Returns:
            The closest point on the annulus to the given point
        """

    def mesh(self) -> AnnulusMeshBuilder:
        """Create a mesh builder for this annulus."""

class Plane2d:
    """An unbounded plane in 2D space.

    The plane forms a separating surface through the origin, stretching infinitely far.
    Defined by a normal direction perpendicular to the plane.

    Example:
        >>> from pybevy.math import Plane2d, Vec2
        >>> # Create a plane with normal pointing up (+Y)
        >>> plane = Plane2d(Vec2(0, 1))
    """

    normal: Dir2

    def __init__(self, normal: Vec2) -> None:
        """Create a new plane from a normal vector.

        Args:
            normal: The normal direction (will be normalized)

        Raises:
            ValueError: If the normal is zero or non-finite
        """

    @staticmethod
    def from_dir(normal: Dir2) -> Plane2d:
        """Create a plane from a normalized direction.

        Args:
            normal: The normal direction (already normalized)
        """

    def __eq__(self, other: object) -> bool: ...

class Line2d:
    """An infinite line going through the origin along a direction in 2D space.

    The line extends infinitely in both the given direction and its opposite direction.
    For a finite line segment, use Segment2d instead.

    Example:
        >>> from pybevy.math import Line2d, Dir2, Vec2
        >>> # Create a horizontal line
        >>> line = Line2d(Dir2(1, 0))
    """

    direction: Dir2

    def __init__(self, direction: Dir2) -> None:
        """Create a new infinite line.

        Args:
            direction: The direction of the line
        """

    def __eq__(self, other: object) -> bool: ...

class Segment2d:
    """A line segment defined by two endpoints in 2D space.

    Example:
        >>> from pybevy.math import Segment2d, Vec2
        >>> # Create a line segment from (0,0) to (1,1)
        >>> segment = Segment2d(Vec2(0, 0), Vec2(1, 1))
    """

    vertices: tuple[Vec2, Vec2]

    def __init__(
        self,
        point1: Vec2 | None = None,
        point2: Vec2 | None = None,
        *,
        vertices: list[Vec2] | None = None,
    ) -> None:
        """Create a new line segment from two endpoints.

        Args:
            point1: The first endpoint
            point2: The second endpoint
            vertices: Alternatively, provide vertices as a list of 2 Vec2
        """

    @staticmethod
    def from_direction_and_length(direction: Dir2, length: float) -> Segment2d:
        """Create a segment centered at origin with given direction and length.

        Args:
            direction: The direction of the segment
            length: The length of the segment
        """

    @staticmethod
    def from_scaled_direction(scaled_direction: Vec2) -> Segment2d:
        """Create a segment centered at origin from a vector.

        The vector represents both direction and length.

        Args:
            scaled_direction: Vector representing direction and length
        """

    @staticmethod
    def from_ray_and_length(ray: Ray2d, length: float) -> Segment2d:
        """Create a segment from a ray origin extending in ray direction.

        Args:
            ray: The starting ray
            length: How far to extend from the ray origin
        """

    def point1(self) -> Vec2:
        """Get the first endpoint of the segment."""

    def point2(self) -> Vec2:
        """Get the second endpoint of the segment."""

    def center(self) -> Vec2:
        """Get the midpoint of the segment."""

    def length(self) -> float:
        """Get the length of the segment."""

    def length_squared(self) -> float:
        """Get the squared length of the segment."""

    def direction(self) -> Dir2:
        """Get the normalized direction from point1 to point2.

        Raises:
            RuntimeError: If endpoints are coincident, NaN, or infinite
        """

    def try_direction(self) -> Dir2:
        """Try to get the normalized direction from point1 to point2.

        Raises:
            ValueError: If a valid direction could not be computed
        """

    def scaled_direction(self) -> Vec2:
        """Get the vector from point1 to point2."""

    def left_normal(self) -> Dir2:
        """Get the normalized counterclockwise normal on the left side.

        Raises:
            RuntimeError: If a valid normal could not be computed
        """

    def try_left_normal(self) -> Dir2:
        """Try to get the left-hand side normal.

        Raises:
            ValueError: If a valid normal could not be computed
        """

    def right_normal(self) -> Dir2:
        """Get the normalized clockwise normal on the right side.

        Raises:
            RuntimeError: If a valid normal could not be computed
        """

    def try_right_normal(self) -> Dir2:
        """Try to get the right-hand side normal.

        Raises:
            ValueError: If a valid normal could not be computed
        """

    def reverse(self) -> None:
        """Reverse the segment in place (swap point1 and point2)."""

    def reversed(self) -> Segment2d:
        """Get a new segment with reversed endpoints."""

    def resized(self, length: float) -> Segment2d:
        """Get a new segment with the same center and direction but a different length."""

    def centered(self) -> Segment2d:
        """Get a new segment centered at the origin."""

    def closest_point(self, point: Vec2) -> Vec2:
        """Get the closest point on the segment to the given point."""

    def rotated(self, rotation: Rot2) -> Segment2d:
        """Get a new segment rotated around the origin by the given rotation."""

    def rotated_around(self, rotation: Rot2, point: Vec2) -> Segment2d:
        """Get a new segment rotated around a point by the given rotation."""

    def rotated_around_center(self, rotation: Rot2) -> Segment2d:
        """Get a new segment rotated around its center by the given rotation."""

    def translated(self, translation: Vec2) -> Segment2d:
        """Get a new segment translated by the given vector."""

    def transformed(self, isometry: Isometry2d) -> Segment2d:
        """Get a new segment transformed by the given isometry."""

    def scaled_left_normal(self) -> Vec2:
        """Get the non-normalized left (counterclockwise) normal scaled by segment length."""

    def scaled_right_normal(self) -> Vec2:
        """Get the non-normalized right (clockwise) normal scaled by segment length."""

    def mesh(self) -> Segment2dMeshBuilder:
        """Create a mesh builder for this segment."""

class Arc2d:
    """A circular arc in 2D space.

    An arc is a portion of a circle defined by a radius and half-angle.
    The arc is centered at the origin and extends symmetrically from the midpoint.

    Example:
        >>> from pybevy.math import Arc2d
        >>> # Create a semicircular arc with radius 2.0
        >>> arc = Arc2d.from_radians(2.0, 3.14159)
        >>> # Create an arc covering 90 degrees
        >>> arc = Arc2d.from_degrees(1.0, 90.0)
        >>> # Create an arc covering one quarter turn
        >>> arc = Arc2d.from_turns(1.0, 0.25)
    """

    radius: float
    half_angle: float

    def __init__(self, radius: float, half_angle: float) -> None:
        """Create a new arc from a radius and half-angle.

        Args:
            radius: The radius of the circle
            half_angle: Half the angle defining the arc (in radians)
        """

    @staticmethod
    def from_radians(radius: float, angle: float) -> Arc2d:
        """Create an arc from a radius and full angle in radians.

        Args:
            radius: The radius of the circle
            angle: The full angle of the arc in radians

        Returns:
            A new Arc2d instance
        """

    @staticmethod
    def from_degrees(radius: float, angle: float) -> Arc2d:
        """Create an arc from a radius and full angle in degrees.

        Args:
            radius: The radius of the circle
            angle: The full angle of the arc in degrees

        Returns:
            A new Arc2d instance
        """

    @staticmethod
    def from_turns(radius: float, fraction: float) -> Arc2d:
        """Create an arc from a radius and a fraction of a full turn.

        For instance, 0.5 turns is a semicircle.

        Args:
            radius: The radius of the circle
            fraction: The fraction of a full turn (1.0 = full circle)

        Returns:
            A new Arc2d instance
        """

    def angle(self) -> float:
        """Get the full angle of the arc in radians.

        Returns:
            The full angle (twice the half_angle)
        """

    def length(self) -> float:
        """Get the arc length.

        Returns:
            The length of the arc along the circle
        """

    def right_endpoint(self) -> Vec2:
        """Get the right-hand endpoint of the arc.

        Returns:
            The position of the right endpoint
        """

    def left_endpoint(self) -> Vec2:
        """Get the left-hand endpoint of the arc.

        Returns:
            The position of the left endpoint
        """

    def endpoints(self) -> tuple[Vec2, Vec2]:
        """Get both endpoints of the arc.

        Returns:
            A tuple of (left_endpoint, right_endpoint)
        """

    def midpoint(self) -> Vec2:
        """Get the midpoint of the arc along the circle.

        Returns:
            The position of the arc's midpoint
        """

    def half_chord_length(self) -> float:
        """Get half the distance between the endpoints.

        Returns:
            Half the length of the chord connecting the endpoints
        """

    def chord_length(self) -> float:
        """Get the distance between the endpoints.

        Returns:
            The length of the straight line connecting the endpoints
        """

    def chord_midpoint(self) -> Vec2:
        """Get the midpoint of the chord connecting the endpoints.

        Returns:
            The position of the chord's midpoint
        """

    def apothem(self) -> float:
        """Get the apothem of the arc.

        The apothem is the distance from the center to the chord midpoint.
        Equivalently, the radius minus the sagitta.

        Note: For a major arc, the apothem will be negative.

        Returns:
            The apothem distance
        """

    def sagitta(self) -> float:
        """Get the sagitta of the arc.

        The sagitta is the distance from the chord midpoint to the arc midpoint.
        Equivalently, the height of the triangle whose base is the chord and
        whose apex is the arc midpoint.

        Returns:
            The sagitta distance
        """

    def is_minor(self) -> bool:
        """Check if the arc is at most half a circle.

        Note: An exact semicircle is both major and minor.

        Returns:
            True if half_angle <= π/2
        """

    def is_major(self) -> bool:
        """Check if the arc is at least half a circle.

        Note: An exact semicircle is both major and minor.

        Returns:
            True if half_angle >= π/2
        """

    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class CircularSector:
    """A circular sector in 2D space (pie slice).

    A circular sector is the region bounded by two radii and an arc.
    It is defined by an arc and includes the area from the center of the
    circle to the arc, forming a "pie slice" shape.

    The sector is positioned so it always includes Vec2.Y and is vertically
    symmetrical. To orient the sector differently, apply a rotation.
    The sector is drawn with the center at the origin.

    Example:
        >>> from pybevy.math import CircularSector
        >>> # Create a semicircular sector with radius 2.0
        >>> sector = CircularSector.from_radians(2.0, 3.14159)
        >>> # Create a quarter-circle sector
        >>> sector = CircularSector.from_degrees(1.0, 90.0)
        >>> # Create a sector covering one third of a circle
        >>> sector = CircularSector.from_turns(1.0, 0.333)
    """

    arc: Arc2d

    def __init__(
        self,
        radius: float = 0.5,
        half_angle: float = 2.0943951023931953,
        *,
        arc: Arc2d | None = None,
    ) -> None:
        """Create a new circular sector from a radius and half-angle.

        Matches Bevy's ``CircularSector::new(radius, half_angle)``.
        The resulting sector spans ``2 * half_angle`` radians.
        For a more intuitive API, use ``from_radians(radius, full_angle)`` instead.

        Args:
            radius: The radius of the circle
            half_angle: Half the angle of the sector in radians
            arc: Alternatively, provide Arc2d directly
        """

    @staticmethod
    def from_radians(radius: float, angle: float) -> CircularSector:
        """Create a circular sector from a radius and full angle in radians.

        Args:
            radius: The radius of the circle
            angle: The full angle of the sector in radians

        Returns:
            A new CircularSector instance
        """

    @staticmethod
    def from_degrees(radius: float, angle: float) -> CircularSector:
        """Create a circular sector from a radius and angle in degrees.

        Args:
            radius: The radius of the circle
            angle: The angle of the sector in degrees

        Returns:
            A new CircularSector instance
        """

    @staticmethod
    def from_turns(radius: float, fraction: float) -> CircularSector:
        """Create a circular sector from a radius and fraction of a turn.

        For instance, 0.5 turns is a semicircle sector.

        Args:
            radius: The radius of the circle
            fraction: The fraction of a full turn (1.0 = full circle)

        Returns:
            A new CircularSector instance
        """

    def radius(self) -> float:
        """Get the radius of the sector.

        Returns:
            The radius of the circle defining the sector
        """

    def half_angle(self) -> float:
        """Get half the angle of the sector.

        Returns:
            Half the sector's angle in radians
        """

    def angle(self) -> float:
        """Get the full angle of the sector.

        Returns:
            The sector's angle in radians
        """

    def arc_length(self) -> float:
        """Get the length of the arc defining the sector.

        Returns:
            The arc length along the circle
        """

    def area(self) -> float:
        """Calculate the area of the sector.

        The area is calculated as: radius² * half_angle

        Returns:
            The area of the circular sector
        """

    def perimeter(self) -> float:
        """Calculate the perimeter of the sector.

        The perimeter includes two radii and the arc length.
        For sectors with angle >= π (half circle or more), returns the
        full circle perimeter (2πr).

        Returns:
            The perimeter of the circular sector
        """

    def half_chord_length(self) -> float:
        """Get half the length of the chord defined by the sector.

        See Arc2d.half_chord_length for details.

        Returns:
            Half the chord length
        """

    def chord_length(self) -> float:
        """Get the length of the chord defined by the sector.

        See Arc2d.chord_length for details.

        Returns:
            The chord length
        """

    def chord_midpoint(self) -> Vec2:
        """Get the midpoint of the chord defined by the sector.

        See Arc2d.chord_midpoint for details.

        Returns:
            The position of the chord's midpoint
        """

    def apothem(self) -> float:
        """Get the apothem of the sector.

        See Arc2d.apothem for details.

        Returns:
            The apothem distance
        """

    def sagitta(self) -> float:
        """Get the sagitta of the sector.

        See Arc2d.sagitta for details.

        Returns:
            The sagitta distance
        """

    def mesh(self) -> CircularSectorMeshBuilder:
        """Create a mesh builder for this circular sector."""

    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class CircularSegment:
    """A circular segment in 2D space.

    A circular segment is the region bounded by an arc and its chord.
    It is defined by an arc and represents the area between the arc and
    the straight line connecting its endpoints.

    Example:
        >>> from pybevy.math import CircularSegment
        >>> # Create a semicircular segment with radius 2.0
        >>> segment = CircularSegment.from_radians(2.0, 3.14159)
        >>> # Create a segment covering 90 degrees
        >>> segment = CircularSegment.from_degrees(1.0, 90.0)
        >>> # Create a segment covering one quarter turn
        >>> segment = CircularSegment.from_turns(1.0, 0.25)
    """

    arc: Arc2d

    def __init__(
        self,
        radius: float = 0.5,
        half_angle: float = 2.0943951023931953,
        *,
        arc: Arc2d | None = None,
    ) -> None:
        """Create a new circular segment from a radius and half-angle.

        Matches Bevy's ``CircularSegment::new(radius, half_angle)``.
        The resulting segment spans ``2 * half_angle`` radians.
        For a more intuitive API, use ``from_radians(radius, full_angle)`` instead.

        Args:
            radius: The radius of the circle
            half_angle: Half the angle of the arc in radians
            arc: Alternatively, provide Arc2d directly
        """

    @staticmethod
    def from_radians(radius: float, angle: float) -> CircularSegment:
        """Create a circular segment from a radius and full angle in radians.

        Args:
            radius: The radius of the circle
            angle: The full angle of the arc in radians

        Returns:
            A new CircularSegment instance
        """

    @staticmethod
    def from_degrees(radius: float, angle: float) -> CircularSegment:
        """Create a circular segment from a radius and full angle in degrees.

        Args:
            radius: The radius of the circle
            angle: The full angle of the arc in degrees

        Returns:
            A new CircularSegment instance
        """

    @staticmethod
    def from_turns(radius: float, fraction: float) -> CircularSegment:
        """Create a circular segment from a radius and a fraction of a full turn.

        For instance, 0.5 turns is a semicircle.

        Args:
            radius: The radius of the circle
            fraction: The fraction of a full turn (1.0 = full circle)

        Returns:
            A new CircularSegment instance
        """

    def half_angle(self) -> float:
        """Get the half-angle of the segment.

        Returns:
            Half the angle of the arc (in radians)
        """

    def angle(self) -> float:
        """Get the full angle of the segment.

        Returns:
            The full angle of the arc (in radians)
        """

    def radius(self) -> float:
        """Get the radius of the segment.

        Returns:
            The radius of the circle
        """

    def arc_length(self) -> float:
        """Get the length of the arc defining the segment.

        Returns:
            The length of the arc along the circle
        """

    def half_chord_length(self) -> float:
        """Get half the length of the segment's base (chord).

        Returns:
            Half the length of the chord connecting the endpoints
        """

    def chord_length(self) -> float:
        """Get the length of the segment's base (chord).

        Returns:
            The length of the straight line connecting the endpoints
        """

    def chord_midpoint(self) -> Vec2:
        """Get the midpoint of the segment's base (chord).

        Returns:
            The position of the chord's midpoint
        """

    def apothem(self) -> float:
        """Get the length of the apothem of this segment.

        The apothem is the signed distance between the segment and the
        center of its circle. For a minor segment (angle < π), this is
        positive. For a major segment, this is negative.

        Returns:
            The apothem distance
        """

    def sagitta(self) -> float:
        """Get the length of the sagitta of this segment (also known as height).

        The sagitta is the distance from the chord midpoint to the arc midpoint.
        It represents the height of the segment.

        Returns:
            The sagitta distance
        """

    def area(self) -> float:
        """Get the area of the circular segment."""

    def perimeter(self) -> float:
        """Get the perimeter of the circular segment (arc length + chord length)."""

    def mesh(self) -> CircularSegmentMeshBuilder:
        """Create a mesh builder for this circular segment."""

    def __eq__(self, other: object) -> bool: ...

class Polygon:
    """A 2D polygon primitive with arbitrary vertices.

    A polygon is defined by a list of vertices connected by edges. Unlike RegularPolygon,
    this can represent any polygon shape, including irregular polygons. The vertices are
    stored as Vec2 points.

    Example:
        >>> from pybevy.math import Polygon, Vec2
        >>> # Create a triangle
        >>> triangle = Polygon([Vec2(0.0, 0.0), Vec2(1.0, 0.0), Vec2(0.5, 1.0)])
        >>> # Create a square
        >>> square = Polygon([Vec2(0.0, 0.0), Vec2(1.0, 0.0), Vec2(1.0, 1.0), Vec2(0.0, 1.0)])
    """

    vertices: list[Vec2]

    def __init__(self, vertices: list[Vec2]) -> None:
        """Create a new polygon from a list of vertices.

        Args:
            vertices: List of Vec2 vertices defining the polygon
        """

    def is_simple(self) -> bool:
        """Test if the polygon is simple.

        A polygon is simple if it is not self-intersecting and not self-tangent.
        No two edges of the polygon may cross each other and each vertex must not
        lie on another edge.

        Returns:
            True if the polygon is simple, False otherwise
        """

    def __eq__(self, other: object) -> bool: ...

class TorusKind:
    """Indicates the kind of torus based on the relationship between minor and major radii."""

    Ring: TorusKind
    """A ring torus (minor_radius < major_radius)."""
    Horn: TorusKind
    """A horn torus (minor_radius == major_radius)."""
    Spindle: TorusKind
    """A spindle torus (minor_radius > major_radius)."""
    Invalid: TorusKind
    """An invalid torus (major_radius <= 0 or minor_radius <= 0)."""

class Torus(Meshable):
    """A 3D torus primitive.

    A torus is a donut-shaped 3D primitive defined by a minor radius (tube radius)
    and a major radius (distance from center to tube center).

    Example:
        >>> from pybevy.math import Torus
        >>> # Create a torus with inner radius 0.5 and outer radius 1.5
        >>> torus = Torus(0.5, 1.5)
        >>> mesh = torus.mesh().build()  # Create mesh from primitive
    """

    @property
    def minor_radius(self) -> float: ...
    @minor_radius.setter
    def minor_radius(self, value: float) -> None: ...
    @property
    def major_radius(self) -> float: ...
    @major_radius.setter
    def major_radius(self, value: float) -> None: ...

    def __init__(
        self,
        inner_radius: float = 0.5,
        outer_radius: float = 1.0,
        *,
        minor_radius: float | None = None,
        major_radius: float | None = None,
    ) -> None:
        """Create a new torus.

        Args:
            inner_radius: The radius of the hole
            outer_radius: The overall radius of the torus
            minor_radius: Alternatively, tube radius directly
            major_radius: Alternatively, ring radius directly
        """

    def inner_radius(self) -> float:
        """Get the inner radius of the torus (radius of the hole)."""

    def outer_radius(self) -> float:
        """Get the outer radius of the torus (overall radius)."""

    def area(self) -> float:
        """Get the surface area of the torus."""

    def volume(self) -> float:
        """Get the volume of the torus."""

    def kind(self) -> TorusKind:
        """Get the kind of this torus.

        Returns:
            Ring if minor_radius < major_radius,
            Horn if minor_radius == major_radius,
            Spindle if minor_radius > major_radius,
            Invalid if either radius is <= 0.
        """

    def mesh(self) -> TorusMeshBuilder:
        """Create a mesh builder for this torus."""

class Tetrahedron(Meshable):
    """A 3D tetrahedron primitive.

    A tetrahedron is a polyhedron with four triangular faces. It's defined by its four vertices.

    Example:
        >>> from pybevy.math import Tetrahedron, Vec3
        >>> tetra = Tetrahedron(
        ...     Vec3(0.0, 0.0, 0.0),
        ...     Vec3(1.0, 0.0, 0.0),
        ...     Vec3(0.5, 1.0, 0.0),
        ...     Vec3(0.5, 0.5, 1.0)
        ... )
        >>> mesh = tetra.mesh().build()  # Create mesh from primitive
    """

    @property
    def vertices(self) -> tuple[Vec3, Vec3, Vec3, Vec3]: ...
    @vertices.setter
    def vertices(self, value: tuple[Vec3, Vec3, Vec3, Vec3] | list[Vec3]) -> None: ...

    def __init__(
        self,
        a: Vec3 | None = None,
        b: Vec3 | None = None,
        c: Vec3 | None = None,
        d: Vec3 | None = None,
        *,
        vertices: list[Vec3] | None = None,
    ) -> None:
        """Create a new tetrahedron from four vertices.

        Args:
            a: First vertex
            b: Second vertex
            c: Third vertex
            d: Fourth vertex
            vertices: Alternatively, provide vertices as a list of 4 Vec3
        """

    def signed_volume(self) -> float:
        """Get the signed volume of the tetrahedron.

        If negative, the normal vector of the face defined by the first three points
        points away from the fourth vertex.
        """

    def centroid(self) -> Vec3:
        """Get the centroid (geometric center) of the tetrahedron."""

    def faces(self) -> tuple[Triangle3d, Triangle3d, Triangle3d, Triangle3d]:
        """Get the four triangular faces of the tetrahedron."""

    def area(self) -> float:
        """Get the surface area of the tetrahedron."""

    def volume(self) -> float:
        """Get the volume of the tetrahedron."""

    def mesh(self) -> TetrahedronMeshBuilder:
        """Create a mesh builder for this tetrahedron."""

class CubicCurve2d:
    """A 2D cubic curve composed of cubic segments.

    A cubic curve is a parametric curve defined by cubic polynomial segments.
    It can be used to represent smooth paths and animations.

    Example:
        >>> from pybevy.math import CubicBezier2d, Vec2
        >>> # Create a Bezier curve and convert it to a CubicCurve2d
        >>> bezier = CubicBezier2d([
        ...     [Vec2(0.0, 0.0), Vec2(1.0, 2.0), Vec2(2.0, 3.0), Vec2(3.0, 0.0)]
        ... ])
        >>> curve = bezier.to_curve()
    """

    segment_count: int

    def position(self, t: float) -> Vec2:
        """Compute the position on the curve at parameter t.

        Args:
            t: Parameter value (0 to segment_count)

        Returns:
            The position on the curve at t
        """

    def velocity(self, t: float) -> Vec2:
        """Compute the velocity (first derivative) on the curve at parameter t.

        Args:
            t: Parameter value (0 to segment_count)

        Returns:
            The velocity vector at t
        """

    def acceleration(self, t: float) -> Vec2:
        """Compute the acceleration (second derivative) on the curve at parameter t.

        Args:
            t: Parameter value (0 to segment_count)

        Returns:
            The acceleration vector at t
        """

    def iter_positions(self, subdivisions: int) -> list[Vec2]:
        """Sample positions along the curve.

        Args:
            subdivisions: Number of segments to divide the curve into

        Returns:
            List of positions sampled along the curve
        """

    def iter_velocities(self, subdivisions: int) -> list[Vec2]:
        """Sample velocities along the curve.

        Args:
            subdivisions: Number of segments to divide the curve into

        Returns:
            List of velocity vectors sampled along the curve
        """

    def iter_accelerations(self, subdivisions: int) -> list[Vec2]:
        """Sample accelerations along the curve.

        Args:
            subdivisions: Number of segments to divide the curve into

        Returns:
            List of acceleration vectors sampled along the curve
        """

class CubicBezier2d:
    """A 2D cubic Bezier curve.

    A Bezier curve is defined by control points. Each segment requires 4 control points.
    The curve passes through the first and last control points of each segment.

    Example:
        >>> from pybevy.math import CubicBezier2d, Vec2
        >>> # Create a single-segment Bezier curve
        >>> bezier = CubicBezier2d([
        ...     [Vec2(0.0, 0.0), Vec2(1.0, 2.0), Vec2(2.0, 3.0), Vec2(3.0, 0.0)]
        ... ])
        >>> curve = bezier.to_curve()
        >>> positions = curve.iter_positions(100)
    """

    def __init__(self, control_points: Sequence[Sequence[Vec2]]) -> None:
        """Create a new cubic Bezier curve.

        Args:
            control_points: List of control point sets. Each set contains 4 Vec2 points.
        """

    def to_curve(self) -> CubicCurve2d:
        """Convert the Bezier curve to a CubicCurve2d for evaluation.

        Returns:
            A CubicCurve2d that can be sampled

        Raises:
            ValueError: If no control points were provided
        """

class CubicCardinalSpline2d:
    """A 2D cubic Cardinal spline (includes Catmull-Rom as special case).

    A Cardinal spline passes through all control points with automatically computed tangents.
    The tension parameter controls how tightly the curve follows the control points.

    Example:
        >>> from pybevy.math import CubicCardinalSpline2d, Vec2
        >>> # Create a Cardinal spline with tension 0.5 (Catmull-Rom)
        >>> points = [Vec2(0.0, 0.0), Vec2(1.0, 1.0), Vec2(2.0, 0.0), Vec2(3.0, 1.0)]
        >>> spline = CubicCardinalSpline2d(0.5, points)
        >>> curve = spline.to_curve()
    """

    tension: float

    def __init__(self, tension: float, control_points: list[Vec2]) -> None:
        """Create a new Cardinal spline.

        Args:
            tension: Tension parameter (0.5 = Catmull-Rom spline)
            control_points: List of points the curve should pass through
        """

    @staticmethod
    def new_catmull_rom(control_points: list[Vec2]) -> CubicCardinalSpline2d:
        """Create a Catmull-Rom spline (Cardinal spline with tension = 0.5).

        Args:
            control_points: List of points the curve should pass through

        Returns:
            A new CubicCardinalSpline2d with tension 0.5
        """

    def to_curve(self) -> CubicCurve2d:
        """Convert the spline to a CubicCurve2d for evaluation.

        Returns:
            A CubicCurve2d that can be sampled

        Raises:
            ValueError: If insufficient control points were provided
        """

class CubicHermite2d:
    """A 2D cubic Hermite curve.

    A Hermite curve is defined by control points and explicit tangent vectors.
    The curve passes through all control points with the specified velocities.

    Example:
        >>> from pybevy.math import CubicHermite2d, Vec2
        >>> # Create a Hermite curve with explicit tangents
        >>> points = [Vec2(0.0, 0.0), Vec2(1.0, 1.0), Vec2(2.0, 0.0)]
        >>> tangents = [Vec2(1.0, 0.0), Vec2(0.0, 1.0), Vec2(-1.0, 0.0)]
        >>> hermite = CubicHermite2d(points, tangents)
        >>> curve = hermite.to_curve()
    """

    def __init__(self, control_points: list[Vec2], tangents: list[Vec2]) -> None:
        """Create a new Hermite curve.

        Args:
            control_points: List of points the curve should pass through
            tangents: List of tangent vectors at each control point
        """

    def to_curve(self) -> CubicCurve2d:
        """Convert the Hermite curve to a CubicCurve2d for evaluation.

        Returns:
            A CubicCurve2d that can be sampled

        Raises:
            ValueError: If insufficient control points were provided
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

    def opposite(self) -> CompassOctant:
        """Get the opposite compass direction."""

    @staticmethod
    def from_index(index: int) -> CompassOctant | None:
        """Get the compass direction from an index (0-7), or None if out of range."""

    def to_index(self) -> int:
        """Get the index (0-7) of this compass direction."""


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
    North: CompassQuadrant
    """North (up), corresponds to Dir2::Y."""

    East: CompassQuadrant
    """East (right), corresponds to Dir2::X."""

    South: CompassQuadrant
    """South (down), corresponds to Dir2::NEG_Y."""

    West: CompassQuadrant
    """West (left), corresponds to Dir2::NEG_X."""

    def opposite(self) -> CompassQuadrant:
        """Get the opposite compass direction."""

    @staticmethod
    def from_index(index: int) -> CompassQuadrant | None:
        """Get the compass direction from an index (0-3), or None if out of range."""

    def to_index(self) -> int:
        """Get the index (0-3) of this compass direction."""


class Rot2:
    """A 2D rotation represented as a unit complex number.

    Rot2 stores the cosine and sine of the rotation angle, enabling
    efficient rotation operations without trigonometric calculations.

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
        rotated = rot.rotate(v)

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

    def __init__(self, *, cos: float | None = None, sin: float | None = None) -> None:
        """Create a rotation from cos/sin values.

        Args:
            cos: Cosine of the angle
            sin: Sine of the angle

        If neither provided, returns identity rotation (cos=1, sin=0).
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
            fraction: Fraction of a full turn (0.5 = 180 degrees)
        """

    @staticmethod
    def from_sin_cos(sin: float, cos: float) -> Rot2:
        """Create a rotation from sine and cosine values.

        Note: The values should form a unit vector (sin² + cos² = 1).
        """

    def as_radians(self) -> float:
        """Get the rotation angle in radians."""

    def as_degrees(self) -> float:
        """Get the rotation angle in degrees."""

    def as_turn_fraction(self) -> float:
        """Get the rotation as a fraction of a full turn."""

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

    def rotate(self, vec: Vec2) -> Vec2:
        """Rotate a 2D vector by this rotation."""

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

    def __mul__(self, other: Rot2) -> Rot2:
        """Compose two rotations."""
