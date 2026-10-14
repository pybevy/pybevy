from typing import ClassVar

from pybevy.collections import LiveArray
from pybevy.math import (
    Dir2,
    Dir3,
    Isometry2d,
    Isometry3d,
    Mat4,
    Quat,
    Rot2,
    Vec2,
    Vec3,
    Vec3A,
    Vec4,
)
from pybevy.mesh import (
    AnnulusMeshBuilder,
    Capsule2dMeshBuilder,
    Capsule3dMeshBuilder,
    CircleMeshBuilder,
    CircularSectorMeshBuilder,
    CircularSegmentMeshBuilder,
    ConeMeshBuilder,
    ConicalFrustumMeshBuilder,
    CuboidMeshBuilder,
    CylinderMeshBuilder,
    EllipseMeshBuilder,
    Meshable,
    PlaneMeshBuilder,
    Polyline2dMeshBuilder,
    Polyline3dMeshBuilder,
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

class Circle(Meshable):
    """A 2D circle primitive.

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

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

class Cone(Meshable):
    """A 3D cone primitive with its tip on +Y.

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

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

class ConicalFrustum(Meshable):
    """A 3D conical frustum primitive.

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

    def __init__(self, *, radius_top: float = 0.25, radius_bottom: float = 0.5, height: float = 0.5) -> None: ...
    @property
    def radius_top(self) -> float: ...
    @radius_top.setter
    def radius_top(self, value: float) -> None: ...
    @property
    def radius_bottom(self) -> float: ...
    @radius_bottom.setter
    def radius_bottom(self, value: float) -> None: ...
    @property
    def height(self) -> float: ...
    @height.setter
    def height(self, value: float) -> None: ...
    def bottom_base(self) -> Circle: ...
    def top_base(self) -> Circle: ...
    def slant_height(self) -> float: ...
    def lateral_area(self) -> float: ...
    def bottom_base_area(self) -> float: ...
    def top_base_area(self) -> float: ...
    def area(self) -> float: ...
    def volume(self) -> float: ...
    def mesh(self) -> ConicalFrustumMeshBuilder: ...

class Rectangle(Meshable):
    """A 2D rectangle primitive.

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

    def __init__(
        self,
        width: float = 1.0,
        height: float = 1.0,
        *,
        half_size: Vec2 = ...
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
    """A 3D sphere primitive.

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

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

    - ``Plane3d(Vec3.Y, half_size=Vec2(w, h))``: horizontal plane.
      ``w`` → world X, ``h`` → world Z.
    - ``Plane3d(Vec3(0,0,1), half_size=Vec2(w, h))``: vertical plane facing +Z.
      ``w`` → world X (width), ``h`` → world Y (height).
      Use this for screens / billboards facing the camera.

    Example::

        # Landscape screen, 6.4 wide x 2.6 tall, facing +Z
        screen = meshes.add(Plane3d(Vec3(0.0, 0.0, 1.0), half_size=Vec2(3.2, 1.3)))

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

    def __init__(
        self, normal: Vec3 = Dir3.Y.as_vec3(), half_size: Vec2 = Vec2.splat(0.5)
    ) -> None:
        """Raise ValueError when ``normal`` is zero or non-finite."""
    @staticmethod
    def from_points(a: Vec3, b: Vec3, c: Vec3) -> tuple[Plane3d, Vec3]:
        """Create a plane from three finite, non-collinear points.

        Raises ValueError when a valid normal cannot be computed.
        """
    @property
    def normal(self) -> Dir3: ...
    @normal.setter
    def normal(self, value: Dir3) -> None: ...
    @property
    def half_size(self) -> Vec2: ...
    @half_size.setter
    def half_size(self, value: Vec2) -> None: ...
    def mesh(self) -> PlaneMeshBuilder: ...

class InfinitePlane3d:
    """An infinite 3D plane defined by its normal direction."""

    def __init__(self, *, normal: Dir3 | Vec3 = ...) -> None:
        """Create a plane through the origin; `normal` defaults to `Dir3.Y`.

        Raises:
            ValueError: If a `Vec3` normal is zero, near zero, or non-finite
        """
    @staticmethod
    def from_points(a: Vec3, b: Vec3, c: Vec3) -> tuple[InfinitePlane3d, Vec3]:
        """Create the plane through three points, returning it with its center."""
    def signed_distance(self, isometry: Isometry3d | Vec3 | Vec3A | Quat, point: Vec3) -> float:
        """Signed distance from the plane to a point."""
    def project_point(self, isometry: Isometry3d | Vec3 | Vec3A | Quat, point: Vec3) -> Vec3:
        """Project a point onto the plane.

        Projection follows the plane's isometry-rotated world normal, including
        when the isometry also translates the plane.
        """
    def isometry_into_xy(self, origin: Vec3) -> Isometry3d:
        """Isometry mapping this plane onto the XY plane."""
    def isometry_from_xy(self, origin: Vec3) -> Isometry3d:
        """Inverse isometry mapping the XY plane onto this plane."""
    def isometries_xy(self, origin: Vec3) -> tuple[Isometry3d, Isometry3d]:
        """Both isometries: plane to XY and back."""
    @property
    def normal(self) -> Dir3:
        """Read-only direction snapshot; assign this property to update the plane."""
    @normal.setter
    def normal(self, value: Dir3) -> None: ...

    def __copy__(self) -> InfinitePlane3d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> InfinitePlane3d:
        """Return an independent copy; the value holds no Python objects."""

class HalfSpace:
    """A region of 3D space defined by a bisecting 2D plane.

    The first 3 components of the Vec4 represent the plane's unit normal,
    and the 4th component is the signed distance from the plane to the origin.
    """

    def normal(self) -> Vec3A:
        """The unit normal of the bisecting plane."""
    def d(self) -> float:
        """The signed distance from the plane to the origin."""
    def normal_d(self) -> Vec4:
        """The normal in xyz and the signed distance in w."""
    @staticmethod
    def intersection_point(a: HalfSpace, b: HalfSpace, c: HalfSpace) -> Vec3 | None:
        """The point where three planes meet, or None when no single point exists."""

    def __init__(self, normal_d: Vec4 = ...) -> None: ...

    def __eq__(self, other: object) -> bool: ...

    def __copy__(self) -> HalfSpace:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> HalfSpace:
        """Return an independent copy; the value holds no Python objects."""

class ViewFrustum:
    """A region of 3D space defined by the intersection of 6 half-spaces.

    Half spaces are ordered left, right, top, bottom, near, far; their normals
    point towards the interior of the frustum. Wrap in a `Frustum` component
    (pybevy.camera) to attach it to an entity.
    """

    NEAR_PLANE_IDX: ClassVar[int]
    FAR_PLANE_IDX: ClassVar[int]

    @property
    def half_spaces(self) -> LiveArray[HalfSpace]:
        """Fixed-length live view of the six half-spaces.

        Reached through a mutable `Frustum` query, `half_spaces[i] = h` writes
        the component; a standalone `ViewFrustum` returns a read-only view, so
        assign all six there. Resizing raises `ValueError`.
        """
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

    def __copy__(self) -> ViewFrustum:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> ViewFrustum:
        """Return an independent copy; the value holds no Python objects."""

class Cylinder(Meshable):
    """A 3D cylinder primitive aligned with the Y axis.

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

    def __init__(
        self,
        radius: float = 0.5,
        height: float = 1.0,
        *,
        half_height: float = ...
    ) -> None: ...
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
    """A 3D cuboid primitive.

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

    def __init__(
        self,
        x_length: float = 1.0,
        y_length: float = 1.0,
        z_length: float = 1.0,
        *,
        half_size: Vec3 = ...
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

class Aabb2d:
    """Axis-aligned bounding box in 2D space.

    An Aabb2d is defined by its minimum and maximum corners, and provides
    efficient collision detection and spatial queries.
    """

    min: Vec2
    max: Vec2

    def __init__(self, center: Vec2, half_size: Vec2) -> None:
        """Create a new Aabb2d from center and half-extents.

        Raises ValueError if half_size has a negative or NaN axis.

        Args:
            center: The center point of the bounding box
            half_size: Half the size of the bounding box in each dimension

        Example:
            ```python
            from pybevy.math import Vec2
            from pybevy.shape import Aabb2d

            aabb = Aabb2d(Vec2(50.0, 50.0), Vec2(50.0, 50.0))
            ```
        """

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
    def intersects(self, volume: Aabb2d | BoundingCircle) -> bool:
        """True if this box intersects an Aabb2d or a BoundingCircle."""

    def merge(self, other: Aabb2d) -> Aabb2d:
        """Merge with another bounding box, returning the smallest box containing both."""

    def grow(self, amount: Vec2) -> Aabb2d:
        """Grow the bounding box by the given amount in all directions.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.
        """

    def shrink(self, amount: Vec2) -> Aabb2d:
        """Shrink the bounding box by the given amount in all directions.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.
        """

    def scale_around_center(self, scale: Vec2) -> Aabb2d:
        """Scale the bounding box around its center.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.
        """

    def visible_area(self) -> float:
        """Calculate the area of the bounding box (width * height)."""

    def bounding_circle(self) -> BoundingCircle:
        """Get the bounding circle that contains this bounding box.

        Raises:
            ValueError: If the computed extents violate Bevy nonnegative preconditions.
        """

    def rotated_by(self, rotation: Rot2 | float) -> Aabb2d:
        """New box rotated around the origin by radians (or Rot2).

        Raises:
            ValueError: If the computed extents violate Bevy nonnegative preconditions.
        """

    def transformed_by(self, translation: Vec2, rotation: Rot2 | float) -> Aabb2d:
        """New box rotated then translated.

        Raises:
            ValueError: If the computed extents violate Bevy nonnegative preconditions.
        """

    def __eq__(self, other: object) -> bool: ...

    def __copy__(self) -> Aabb2d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Aabb2d:
        """Return an independent copy; the value holds no Python objects."""

    def translated_by(self, translation: Vec2) -> Aabb2d:
        """A copy moved by `translation`."""
    def translate_by(self, translation: Vec2) -> None:
        """Move this volume in place by `translation`."""
    def rotate_by(self, rotation: Rot2 | float) -> None:
        """Rotate this volume in place around the origin, as `rotated_by` does."""
    def transform_by(self, translation: Vec2, rotation: Rot2 | float) -> None:
        """Rotate this volume around the origin, then translate it, in place."""

class BoundingCircle:
    """Bounding circle in 2D space.

    A BoundingCircle is defined by its center point and radius, providing
    efficient circular collision detection.
    """

    center: Vec2

    def __init__(self, center: Vec2, radius: float) -> None:
        """Create a new BoundingCircle.

        Raises ValueError if radius is negative or NaN.

        Args:
            center: The center point of the circle
            radius: The radius of the circle

        Example:
            ```python
            from pybevy.math import Vec2
            from pybevy.shape import BoundingCircle

            # Create a circle with radius 50 centered at (100, 100)
            circle = BoundingCircle(Vec2(100.0, 100.0), 50.0)
            ```
        """

    @staticmethod
    def from_point_cloud(isometry: Isometry2d, points: list[Vec2]) -> BoundingCircle:
        """Create a BoundingCircle that contains all given points.

        Args:
            isometry: Transform to apply to points
            points: List of Vec2 points to bound

        Returns:
            A BoundingCircle containing all transformed points, centered on
            their centroid with the farthest point on its boundary. Bevy does
            not promise the smallest such circle.
        """

    def radius(self) -> float:
        """Get the radius of the circle."""

    def closest_point(self, point: Vec2) -> Vec2:
        """Find the closest point on the circle to the given point."""

    def contains(self, other: BoundingCircle) -> bool:
        """Check if this circle completely contains another circle."""

    def merge(self, other: BoundingCircle) -> BoundingCircle:
        """Merge with another circle, returning the smallest circle containing both.

        Raises:
            ValueError: If the computed extents violate Bevy nonnegative preconditions.
        """

    def grow(self, amount: float) -> BoundingCircle:
        """Grow the circle radius by the given amount.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.
        """

    def shrink(self, amount: float) -> BoundingCircle:
        """Shrink the circle radius by the given amount.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.
        """

    def scale_around_center(self, scale: float) -> BoundingCircle:
        """Scale the circle around its center.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.
        """

    def visible_area(self) -> float:
        """Calculate the area of the circle (pi * r^2)."""

    def aabb_2d(self) -> Aabb2d:
        """Get the axis-aligned bounding box that contains this circle."""

    def rotated_by(self, rotation: Rot2 | float) -> BoundingCircle:
        """New circle rotated around the origin by radians (or Rot2)."""

    def transformed_by(self, translation: Vec2, rotation: Rot2 | float) -> BoundingCircle:
        """New circle rotated then translated."""

    def intersects(self, volume: Aabb2d | BoundingCircle) -> bool:
        """True if this circle intersects an Aabb2d or a BoundingCircle."""

    def __eq__(self, other: object) -> bool: ...

    def __copy__(self) -> BoundingCircle:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> BoundingCircle:
        """Return an independent copy; the value holds no Python objects."""

    def translated_by(self, translation: Vec2) -> BoundingCircle:
        """A copy moved by `translation`."""
    def translate_by(self, translation: Vec2) -> None:
        """Move this volume in place by `translation`."""
    def rotate_by(self, rotation: Rot2 | float) -> None:
        """Rotate this volume in place around the origin, as `rotated_by` does."""
    def transform_by(self, translation: Vec2, rotation: Rot2 | float) -> None:
        """Rotate this volume around the origin, then translate it, in place."""

class Aabb3d:
    """Axis-aligned bounding box in 3D space."""

    min: Vec3A
    max: Vec3A

    def __init__(self, center: Vec3A | Vec3, half_size: Vec3A | Vec3) -> None:
        """Create a new Aabb3d from a center point and half-extents.

        Raises ValueError if half_size has a negative or NaN axis.

        Args:
            center: The center point of the box
            half_size: The half-extents (half the width, height, depth)

        Example:
            >>> from pybevy.math import Vec3
            >>> from pybevy.shape import Aabb3d
            >>> aabb = Aabb3d(Vec3(100.0, 100.0, 100.0), Vec3(25.0, 25.0, 25.0))
        """

    @staticmethod
    def from_min_max(min: Vec3A | Vec3, max: Vec3A | Vec3) -> Aabb3d:
        """Create an Aabb3d directly from minimum and maximum corners.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.

        Args:
            min: The minimum corner of the bounding box
            max: The maximum corner of the bounding box

        Example:
            >>> from pybevy.math import Vec3
            >>> from pybevy.shape import Aabb3d
            >>> aabb = Aabb3d.from_min_max(Vec3(0.0, 0.0, 0.0), Vec3(100.0, 100.0, 100.0))
        """

    def center(self) -> Vec3A:
        """Center of the bounding box."""

    def half_size(self) -> Vec3A:
        """Half-extents of the bounding box."""

    def closest_point(self, point: Vec3A | Vec3) -> Vec3A:
        """Find the closest point on the AABB to the given point."""

    def contains(self, other: Aabb3d) -> bool:
        """Check if this AABB contains another AABB."""

    def merge(self, other: Aabb3d) -> Aabb3d:
        """Merge this AABB with another, returning the smallest AABB containing both."""

    def grow(self, amount: Vec3A | Vec3) -> Aabb3d:
        """Grow the AABB by the given amount in each direction.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.
        """

    def shrink(self, amount: Vec3A | Vec3) -> Aabb3d:
        """Shrink the AABB by the given amount in each direction.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.
        """

    def scale_around_center(self, scale: Vec3A | Vec3) -> Aabb3d:
        """Scale the AABB around its center.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.
        """

    def visible_area(self) -> float:
        """Return half the surface area (Bevy's visible-area heuristic)."""

    def bounding_sphere(self) -> BoundingSphere:
        """Get the bounding sphere that contains this AABB.

        Raises:
            ValueError: If the computed extents violate Bevy nonnegative preconditions.
        """

    def rotated_by(self, rotation: Quat) -> Aabb3d:
        """New box rotated around the origin.

        Raises:
            ValueError: If the computed extents violate Bevy nonnegative preconditions.
        """

    def transformed_by(self, translation: Vec3A | Vec3, rotation: Quat) -> Aabb3d:
        """New box rotated then translated.

        Raises:
            ValueError: If the computed extents violate Bevy nonnegative preconditions.
        """

    def intersects(self, volume: Aabb3d | BoundingSphere) -> bool:
        """True if this box intersects an Aabb3d or a BoundingSphere."""

    @staticmethod
    def from_point_cloud(isometry: Isometry3d, points: list[Vec3]) -> Aabb3d:
        """Create an Aabb3d from a collection of points.

        Args:
            isometry: Transformation to apply to the points
            points: List of Vec3 points to create the bounding box from

        Example:
            >>> from pybevy.math import Isometry3d, Vec3
            >>> from pybevy.shape import Aabb3d
            >>> points = [Vec3(0.0, 0.0, 0.0), Vec3(10.0, 10.0, 10.0)]
            >>> aabb = Aabb3d.from_point_cloud(Isometry3d.IDENTITY, points)
        """

    def __eq__(self, other: object) -> bool: ...

    def __copy__(self) -> Aabb3d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Aabb3d:
        """Return an independent copy; the value holds no Python objects."""

    def translated_by(self, translation: Vec3A | Vec3) -> Aabb3d:
        """A copy moved by `translation`."""
    def translate_by(self, translation: Vec3A | Vec3) -> None:
        """Move this volume in place by `translation`."""
    def rotate_by(self, rotation: Quat) -> None:
        """Rotate this volume in place around the origin, as `rotated_by` does."""
    def transform_by(self, translation: Vec3A | Vec3, rotation: Quat) -> None:
        """Rotate this volume around the origin, then translate it, in place."""

class BoundingSphere:
    """Bounding sphere in 3D space."""

    center: Vec3A

    def __init__(self, center: Vec3A | Vec3, radius: float) -> None:
        """Create a new BoundingSphere from a center point and radius.

        Raises ValueError if radius is negative or NaN.

        Args:
            center: The center point of the sphere
            radius: The radius of the sphere

        Example:
            >>> from pybevy.math import Vec3
            >>> from pybevy.shape import BoundingSphere
            >>> sphere = BoundingSphere(Vec3(100.0, 100.0, 100.0), 50.0)
        """

    def radius(self) -> float:
        """Radius of the bounding sphere."""

    def closest_point(self, point: Vec3A | Vec3) -> Vec3A:
        """Find the closest point on the sphere to the given point.

        Exterior points are projected onto the surface along the direction from
        the sphere's center. Interior and boundary points are returned unchanged.
        """

    def contains(self, other: BoundingSphere) -> bool:
        """Check if this sphere contains another sphere."""

    def merge(self, other: BoundingSphere) -> BoundingSphere:
        """Merge this sphere with another, returning the smallest sphere containing both.

        Raises:
            ValueError: If the computed extents violate Bevy nonnegative preconditions.
        """

    def grow(self, amount: float) -> BoundingSphere:
        """Grow the sphere by the given amount.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.
        """

    def shrink(self, amount: float) -> BoundingSphere:
        """Shrink the sphere by the given amount.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.
        """

    def scale_around_center(self, scale: float) -> BoundingSphere:
        """Scale the sphere around its center.

        Raises:
            ValueError: If the operation violates Bevy nonnegative extent preconditions.
        """

    def visible_area(self) -> float:
        """Return half the surface area (Bevy's visible-area heuristic)."""

    def aabb_3d(self) -> Aabb3d:
        """Get the axis-aligned bounding box that contains this sphere."""

    def rotated_by(self, rotation: Quat) -> BoundingSphere:
        """New sphere rotated around the origin."""

    def transformed_by(self, translation: Vec3A | Vec3, rotation: Quat) -> BoundingSphere:
        """New sphere rotated then translated."""

    def intersects(self, volume: Aabb3d | BoundingSphere) -> bool:
        """True if this sphere intersects an Aabb3d or a BoundingSphere."""

    @staticmethod
    def from_point_cloud(isometry: Isometry3d, points: list[Vec3]) -> BoundingSphere:
        """Create a BoundingSphere from a collection of points.

        Args:
            isometry: Transformation to apply to the points
            points: List of Vec3 points to create the bounding sphere from

        Example:
            >>> from pybevy.math import Isometry3d, Vec3
            >>> from pybevy.shape import BoundingSphere
            >>> points = [Vec3(0.0, 0.0, 0.0), Vec3(10.0, 10.0, 10.0)]
            >>> sphere = BoundingSphere.from_point_cloud(Isometry3d.IDENTITY, points)
        """

    def __eq__(self, other: object) -> bool: ...

    def __copy__(self) -> BoundingSphere:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> BoundingSphere:
        """Return an independent copy; the value holds no Python objects."""

    def translated_by(self, translation: Vec3A | Vec3) -> BoundingSphere:
        """A copy moved by `translation`."""
    def translate_by(self, translation: Vec3A | Vec3) -> None:
        """Move this volume in place by `translation`."""
    def rotate_by(self, rotation: Quat) -> None:
        """Rotate this volume in place around the origin, as `rotated_by` does."""
    def transform_by(self, translation: Vec3A | Vec3, rotation: Quat) -> None:
        """Rotate this volume around the origin, then translate it, in place."""

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
            >>> from pybevy.math import Dir2, Vec2
            >>> from pybevy.shape import Ray2d
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

    def __copy__(self) -> Ray2d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Ray2d:
        """Return an independent copy; the value holds no Python objects."""

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
            >>> from pybevy.math import Dir3, Vec3
            >>> from pybevy.shape import Ray3d
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

    def __copy__(self) -> Ray3d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Ray3d:
        """Return an independent copy; the value holds no Python objects."""

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
            >>> from pybevy.math import Dir2, Vec2
            >>> from pybevy.shape import RayCast2d
            >>> raycast = RayCast2d(Vec2(0.0, 0.0), Dir2.X, 100.0)
        """

    @staticmethod
    def from_ray(ray: Ray2d, max: float) -> RayCast2d:
        """Create a RayCast2d from a Ray2d and maximum distance."""

    def intersects(self, volume: Aabb2d | BoundingCircle) -> bool:
        """True if the ray hits an Aabb2d or a BoundingCircle within `max`."""

    def aabb_intersection_at(self, aabb: Aabb2d) -> float | None:
        """Get the distance to intersection with Aabb2d, or None."""

    def circle_intersection_at(self, circle: BoundingCircle) -> float | None:
        """Get the distance to intersection with BoundingCircle, or None."""

    def direction_recip(self) -> Vec2:
        """Get the reciprocal of the ray direction (1.0 / direction for each component)."""

    def __copy__(self) -> RayCast2d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> RayCast2d:
        """Return an independent copy; the value holds no Python objects."""

class RayCast3d:
    """A 3D raycast with a maximum distance for intersection testing."""

    max: float

    @property
    def origin(self) -> Vec3A:
        """The ray origin; assign a new value to move the ray."""

    @origin.setter
    def origin(self, value: Vec3A | Vec3) -> None: ...

    @property
    def direction(self) -> Dir3:
        """A read-only snapshot of the ray direction; assign a new Dir3 to turn the ray."""

    @direction.setter
    def direction(self, value: Dir3) -> None: ...

    def __init__(self, origin: Vec3A | Vec3, direction: Dir3, max: float) -> None:
        """Create a new RayCast3d.

        Args:
            origin: Starting point of the ray
            direction: Normalized direction vector
            max: Maximum distance for the raycast

        Example:
            >>> from pybevy.math import Dir3, Vec3
            >>> from pybevy.shape import RayCast3d
            >>> raycast = RayCast3d(Vec3(0.0, 0.0, 0.0), Dir3.X, 100.0)
        """

    @staticmethod
    def from_ray(ray: Ray3d, max: float) -> RayCast3d:
        """Create a RayCast3d from a Ray3d and maximum distance."""

    def intersects(self, volume: Aabb3d | BoundingSphere) -> bool:
        """True if the ray hits an Aabb3d or a BoundingSphere within `max`."""

    def aabb_intersection_at(self, aabb: Aabb3d) -> float | None:
        """Get the distance to intersection with Aabb3d, or None."""

    def sphere_intersection_at(self, sphere: BoundingSphere) -> float | None:
        """Get the distance to intersection with BoundingSphere, or None."""

    def direction_recip(self) -> Vec3:
        """Get the reciprocal of the ray direction (1.0 / direction for each component)."""

    def __copy__(self) -> RayCast3d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> RayCast3d:
        """Return an independent copy; the value holds no Python objects."""

class Capsule2d(Meshable):
    """A 2D capsule primitive (pill shape).

    A capsule is a shape consisting of a rectangle with semicircular ends.
    It's defined by a radius and a length (distance between the centers of the semicircles).

    Example:
        >>> from pybevy.shape import Capsule2d
        >>> # Create a capsule with radius 5.0 and length 20.0
        >>> capsule = Capsule2d(5.0, 20.0)
        >>> mesh = capsule.mesh().build()  # Create mesh from primitive

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

    @property
    def radius(self) -> float: ...
    @radius.setter
    def radius(self, value: float) -> None: ...
    @property
    def half_length(self) -> float: ...
    @half_length.setter
    def half_length(self, value: float) -> None: ...

    def __init__(
        self,
        radius: float = 0.5,
        length: float = 1.0,
        *,
        half_length: float = ...
    ) -> None:
        """Create a new 2D capsule.

        Args:
            radius: The radius of the semicircular ends
            length: The distance between the centers of the semicircles
        """

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
        >>> from pybevy.math import Vec2
        >>> from pybevy.shape import Ellipse
        >>> # Create an ellipse with semi-axes of 3.0 and 2.0
        >>> ellipse = Ellipse(half_size=Vec2(3.0, 2.0))
        >>> mesh = ellipse.mesh().build()  # Create mesh from primitive

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

    @property
    def half_size(self) -> Vec2: ...
    @half_size.setter
    def half_size(self, value: Vec2) -> None: ...

    def __init__(self, *, half_size: Vec2 = ...) -> None:
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

class Polyline2d(Meshable):
    """A 2D polyline: an open chain of connected line segments."""

    def __init__(self, vertices: list[Vec2] = ...) -> None:
        """Create a polyline through the vertices, or use Bevy's default endpoints."""

    @staticmethod
    def with_subdivisions(start: Vec2, end: Vec2, subdivisions: int) -> Polyline2d:
        """Create a polyline from `start` to `end` with subdivision points."""

    @property
    def vertices(self) -> list[Vec2]: ...
    @vertices.setter
    def vertices(self, value: list[Vec2]) -> None: ...

    def mesh(self) -> Polyline2dMeshBuilder:
        """Create a mesh builder for this polyline."""

class Polyline3d(Meshable):
    """A 3D polyline: an open chain of connected line segments."""

    def __init__(self, vertices: list[Vec3] = ...) -> None:
        """Create a polyline through the vertices, or use Bevy's default endpoints."""

    @staticmethod
    def with_subdivisions(start: Vec3, end: Vec3, subdivisions: int) -> Polyline3d:
        """Create a polyline from `start` to `end` with subdivision points."""

    @property
    def vertices(self) -> list[Vec3]: ...
    @vertices.setter
    def vertices(self, value: list[Vec3]) -> None: ...

    def mesh(self) -> Polyline3dMeshBuilder:
        """Create a mesh builder for this polyline."""

class Triangle2d(Meshable):
    """A 2D triangle primitive.

    A triangle is defined by its three vertices.

    Example:
        >>> from pybevy.math import Vec2
        >>> from pybevy.shape import Triangle2d
        >>> tri = Triangle2d(
        ...     Vec2(0.0, 0.0),
        ...     Vec2(1.0, 0.0),
        ...     Vec2(0.5, 1.0)
        ... )
        >>> mesh = tri.mesh().build()  # Create mesh from primitive
    """

    @property
    def vertices(self) -> list[Vec2]: ...
    @vertices.setter
    def vertices(self, value: tuple[Vec2, Vec2, Vec2] | list[Vec2]) -> None: ...

    def __init__(
        self,
        a: Vec2 = Vec2(0.0, 0.5),
        b: Vec2 = Vec2(-0.5, -0.5),
        c: Vec2 = Vec2(0.5, -0.5),
        *,
        vertices: list[Vec2] = ...
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
        >>> from pybevy.shape import Capsule3d
        >>> # Create a capsule with radius 0.5 and length 2.0
        >>> capsule = Capsule3d(0.5, 2.0)
        >>> mesh = capsule.mesh().build()  # Create mesh from primitive

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

    @property
    def radius(self) -> float: ...
    @radius.setter
    def radius(self, value: float) -> None: ...
    @property
    def half_length(self) -> float: ...
    @half_length.setter
    def half_length(self, value: float) -> None: ...

    def __init__(
        self, radius: float = 0.5, length: float = 1.0, *, half_length: float = ...
    ) -> None:
        """Create a new 3D capsule.

        Args:
            radius: The radius of the hemispherical ends and cylindrical body
            length: The distance between the centers of the hemispheres
            half_length: Half that distance; mutually exclusive with length
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
        >>> from pybevy.math import Vec3
        >>> from pybevy.shape import Triangle3d
        >>> tri = Triangle3d(
        ...     Vec3(0.0, 0.0, 0.0),
        ...     Vec3(1.0, 0.0, 0.0),
        ...     Vec3(0.5, 1.0, 0.0)
        ... )
        >>> mesh = tri.mesh().build()  # Create mesh from primitive
    """

    @property
    def vertices(self) -> list[Vec3]: ...
    @vertices.setter
    def vertices(self, value: tuple[Vec3, Vec3, Vec3] | list[Vec3]) -> None: ...

    def __init__(
        self,
        a: Vec3 = Vec3(0.0, 0.5, 0.0),
        b: Vec3 = Vec3(-0.5, -0.5, 0.0),
        c: Vec3 = Vec3(0.5, -0.5, 0.0),
        *,
        vertices: list[Vec3] = ...
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

    def largest_side(self) -> tuple[Vec3, Vec3]:
        """Get the endpoints of the largest side of the triangle."""

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
        >>> from pybevy.shape import RegularPolygon
        >>> # Create a hexagon with circumradius 1.0
        >>> hexagon = RegularPolygon(1.0, 6)
        >>> mesh = hexagon.mesh().build()  # Create mesh from primitive

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

    @property
    def circumcircle(self) -> Circle: ...
    @circumcircle.setter
    def circumcircle(self, value: Circle) -> None: ...
    @property
    def sides(self) -> int: ...
    @sides.setter
    def sides(self, value: int) -> None: ...

    def __init__(self, circumradius: float = 0.5, sides: int = 6) -> None:
        """Create a new regular polygon.

        Args:
            circumradius: The radius of the circumcircle (circle passing through all vertices)
            sides: The number of sides (must be at least 3)

        Raises:
            ValueError: If circumradius is non-finite or negative, or sides is less than 3.
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
        >>> from pybevy.shape import Rhombus
        >>> # Create a rhombus with horizontal diagonal 2.0 and vertical diagonal 3.0
        >>> rhombus = Rhombus(2.0, 3.0)
        >>> mesh = rhombus.mesh().build()  # Create mesh from primitive

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
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
        half_diagonals: Vec2 = ...
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
        >>> from pybevy.shape import Annulus
        >>> # Create an annulus with inner radius 0.5 and outer radius 1.0
        >>> ring = Annulus(0.5, 1.0)
        >>> mesh = ring.mesh().build()  # Create mesh from primitive

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension; zero is allowed. Bevy accepts any value.
    """

    inner_circle: Circle
    outer_circle: Circle

    def __init__(
        self,
        inner_radius: float = 0.5,
        outer_radius: float = 1.0,
        *,
        inner_circle: Circle = ...,
        outer_circle: Circle = ...
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

        The exact center maps deterministically to the positive-X point on the
        inner boundary when the inner radius is positive.

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
        >>> from pybevy.math import Vec2
        >>> from pybevy.shape import Plane2d
        >>> # Create a plane with normal pointing up (+Y)
        >>> plane = Plane2d(Vec2(0, 1))
    """

    @property
    def normal(self) -> Dir2:
        """Read-only direction snapshot; assign this property to update the plane."""
    @normal.setter
    def normal(self, value: Dir2) -> None: ...

    def __init__(self, normal: Dir2 | Vec2 = ...) -> None:
        """Create a new plane; `normal` defaults to `Dir2.Y`.

        Args:
            normal: A direction, or a vector that is normalized

        Raises:
            ValueError: If a `Vec2` normal is zero or non-finite
        """

    def __eq__(self, other: object) -> bool: ...

    def __copy__(self) -> Plane2d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Plane2d:
        """Return an independent copy; the value holds no Python objects."""

class Line2d:
    """An infinite line going through the origin along a direction in 2D space.

    The line extends infinitely in both the given direction and its opposite direction.
    For a finite line segment, use Segment2d instead.

    Example:
        >>> from pybevy.math import Dir2, Vec2
        >>> from pybevy.shape import Line2d
        >>> # Create a horizontal line
        >>> line = Line2d(direction=Dir2(1, 0))
    """

    direction: Dir2

    def __init__(self, *, direction: Dir2) -> None:
        """Create a new infinite line.

        Args:
            direction: The direction of the line
        """

    def __eq__(self, other: object) -> bool: ...

    def __copy__(self) -> Line2d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Line2d:
        """Return an independent copy; the value holds no Python objects."""

class Line3d:
    """An infinite line going through the origin along a direction in 3D space.

    The line extends infinitely in both the given direction and its opposite direction.

    Example:
        >>> from pybevy.math import Dir3
        >>> from pybevy.shape import Line3d
        >>> # Create a line along the X axis
        >>> line = Line3d(direction=Dir3(1, 0, 0))
    """

    direction: Dir3

    def __init__(self, *, direction: Dir3) -> None:
        """Create a new infinite line.

        Args:
            direction: The direction of the line
        """

    def __eq__(self, other: object) -> bool: ...

    def __copy__(self) -> Line3d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Line3d:
        """Return an independent copy; the value holds no Python objects."""

class Segment2d(Meshable):
    """A line segment defined by two endpoints in 2D space.

    Example:
        >>> from pybevy.math import Vec2
        >>> from pybevy.shape import Segment2d
        >>> # Create a line segment from (0,0) to (1,1)
        >>> segment = Segment2d(Vec2(0, 0), Vec2(1, 1))
    """

    @property
    def vertices(self) -> list[Vec2]: ...
    @vertices.setter
    def vertices(self, value: tuple[Vec2, Vec2] | list[Vec2]) -> None: ...

    def __init__(
        self,
        point1: Vec2 = Vec2(-0.5, 0.0),
        point2: Vec2 = Vec2(0.5, 0.0),
        *,
        vertices: list[Vec2] = ...
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
            RuntimeError: If endpoints are coincident, NaN, or infinite
        """

    def try_left_normal(self) -> Dir2:
        """Try to get the left-hand side normal.

        Raises:
            ValueError: If a valid normal could not be computed
        """

    def right_normal(self) -> Dir2:
        """Get the normalized clockwise normal on the right side.

        Raises:
            RuntimeError: If endpoints are coincident, NaN, or infinite
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
        >>> from pybevy.shape import Arc2d
        >>> # Create a semicircular arc with radius 2.0
        >>> arc = Arc2d.from_radians(2.0, 3.14159)
        >>> # Create an arc covering 90 degrees
        >>> arc = Arc2d.from_degrees(1.0, 90.0)
        >>> # Create an arc covering one quarter turn
        >>> arc = Arc2d.from_turns(1.0, 0.25)

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite radius or a non-finite angle; zero is allowed. Bevy accepts any value.
    """

    radius: float
    half_angle: float

    def __init__(self, radius: float = 0.5, half_angle: float = ...) -> None:
        """Create a new arc from a radius and half-angle.

        Defaults to Bevy's: radius 0.5 and a half-angle of ``2 * pi / 3``,
        covering two thirds of a circle.

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

    def endpoints(self) -> list[Vec2]:
        """Get both endpoints of the arc.

        Returns:
            A list containing the left and right endpoints
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

    def __copy__(self) -> Arc2d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Arc2d:
        """Return an independent copy; the value holds no Python objects."""

class CircularSector:
    """A circular sector in 2D space (pie slice).

    A circular sector is the region bounded by two radii and an arc.
    It is defined by an arc and includes the area from the center of the
    circle to the arc, forming a "pie slice" shape.

    The sector is positioned so it always includes Vec2.Y and is vertically
    symmetrical. To orient the sector differently, apply a rotation.
    The sector is drawn with the center at the origin.

    Example:
        >>> from pybevy.shape import CircularSector
        >>> # Create a semicircular sector with radius 2.0
        >>> sector = CircularSector.from_radians(2.0, 3.14159)
        >>> # Create a quarter-circle sector
        >>> sector = CircularSector.from_degrees(1.0, 90.0)
        >>> # Create a sector covering one third of a circle
        >>> sector = CircularSector.from_turns(1.0, 0.333)

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite radius or a non-finite angle; zero is allowed. Bevy accepts any value.
    """

    arc: Arc2d

    def __init__(
        self,
        radius: float = 0.5,
        half_angle: float = 2.0943951023931953,
        *,
        arc: Arc2d = ...
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

        The perimeter includes two radii and the arc length. Only a full
        circle (angle >= 2*pi) returns the circle perimeter (2*pi*r) instead.

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
        >>> from pybevy.shape import CircularSegment
        >>> # Create a semicircular segment with radius 2.0
        >>> segment = CircularSegment.from_radians(2.0, 3.14159)
        >>> # Create a segment covering 90 degrees
        >>> segment = CircularSegment.from_degrees(1.0, 90.0)
        >>> # Create a segment covering one quarter turn
        >>> segment = CircularSegment.from_turns(1.0, 0.25)

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite radius or a non-finite angle; zero is allowed. Bevy accepts any value.
    """

    arc: Arc2d

    def __init__(
        self,
        radius: float = 0.5,
        half_angle: float = 2.0943951023931953,
        *,
        arc: Arc2d = ...
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
        >>> from pybevy.math import Vec2
        >>> from pybevy.shape import Polygon
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

    def __copy__(self) -> Polygon:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Polygon:
        """Return an independent copy; the value holds no Python objects."""

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
        >>> from pybevy.shape import Torus
        >>> # Create a torus with inner radius 0.5 and outer radius 1.5
        >>> torus = Torus(0.5, 1.5)
        >>> mesh = torus.mesh().build()  # Create mesh from primitive

    Raises ValueError from constructors and setters for a negative, NaN, or
    infinite dimension, including an inner radius above the outer radius;
    zero is allowed. Bevy accepts any value.
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
        minor_radius: float = ...,
        major_radius: float = ...
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
        >>> from pybevy.math import Vec3
        >>> from pybevy.shape import Tetrahedron
        >>> tetra = Tetrahedron(
        ...     Vec3(0.0, 0.0, 0.0),
        ...     Vec3(1.0, 0.0, 0.0),
        ...     Vec3(0.5, 1.0, 0.0),
        ...     Vec3(0.5, 0.5, 1.0)
        ... )
        >>> mesh = tetra.mesh().build()  # Create mesh from primitive
    """

    @property
    def vertices(self) -> list[Vec3]: ...
    @vertices.setter
    def vertices(self, value: tuple[Vec3, Vec3, Vec3, Vec3] | list[Vec3]) -> None: ...

    def __init__(
        self,
        a: Vec3 = Vec3(0.5, 0.5, 0.5),
        b: Vec3 = Vec3(-0.5, 0.5, -0.5),
        c: Vec3 = Vec3(-0.5, -0.5, 0.5),
        d: Vec3 = Vec3(0.5, -0.5, -0.5),
        *,
        vertices: list[Vec3] = ...
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

    def faces(self) -> list[Triangle3d]:
        """Get the four triangular faces of the tetrahedron."""

    def area(self) -> float:
        """Get the surface area of the tetrahedron."""

    def volume(self) -> float:
        """Get the volume of the tetrahedron."""

    def mesh(self) -> TetrahedronMeshBuilder:
        """Create a mesh builder for this tetrahedron."""

class AabbCast2d:
    """A 2D AABB swept along a ray for collision testing."""

    def __init__(self, aabb: Aabb2d, origin: Vec2, direction: Dir2, max: float) -> None:
        """Cast an `Aabb2d` from an origin along a direction for at most `max` distance."""

    @staticmethod
    def from_ray(aabb: Aabb2d, ray: Ray2d, max: float) -> AabbCast2d:
        """Cast an `Aabb2d` along a `Ray2d` for at most `max` distance."""

    def aabb_collision_at(self, aabb: Aabb2d) -> float | None:
        """Distance at which the swept AABB first hits `aabb`, or None."""

    def intersects(self, volume: Aabb2d) -> bool:
        """True if the swept AABB hits `volume`."""

    @property
    def aabb(self) -> Aabb2d:
        """The AABB being cast."""

    @aabb.setter
    def aabb(self, value: Aabb2d) -> None: ...

    @property
    def ray(self) -> RayCast2d:
        """The ray along which the AABB is cast."""

    @ray.setter
    def ray(self, value: RayCast2d) -> None: ...

    def __copy__(self) -> AabbCast2d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> AabbCast2d:
        """Return an independent copy; the value holds no Python objects."""

class BoundingCircleCast:
    """A 2D circle swept along a ray for collision testing."""

    def __init__(self, circle: BoundingCircle, origin: Vec2, direction: Dir2, max: float) -> None:
        """Cast a `BoundingCircle` from an origin along a direction for at most `max` distance."""

    @staticmethod
    def from_ray(circle: BoundingCircle, ray: Ray2d, max: float) -> BoundingCircleCast:
        """Cast a `BoundingCircle` along a `Ray2d` for at most `max` distance."""

    def circle_collision_at(self, circle: BoundingCircle) -> float | None:
        """Distance at which the swept circle first hits `circle`, or None."""

    def intersects(self, volume: BoundingCircle) -> bool:
        """True if the swept circle hits `volume`."""

    @property
    def circle(self) -> BoundingCircle:
        """The circle being cast."""

    @circle.setter
    def circle(self, value: BoundingCircle) -> None: ...

    @property
    def ray(self) -> RayCast2d:
        """The ray along which the circle is cast."""

    @ray.setter
    def ray(self, value: RayCast2d) -> None: ...

    def __copy__(self) -> BoundingCircleCast:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> BoundingCircleCast:
        """Return an independent copy; the value holds no Python objects."""

class AabbCast3d:
    """A 3D AABB swept along a ray for collision testing."""

    def __init__(self, aabb: Aabb3d, origin: Vec3A | Vec3, direction: Dir3, max: float) -> None:
        """Cast an `Aabb3d` from an origin along a direction for at most `max` distance."""

    @staticmethod
    def from_ray(aabb: Aabb3d, ray: Ray3d, max: float) -> AabbCast3d:
        """Cast an `Aabb3d` along a `Ray3d` for at most `max` distance."""

    def aabb_collision_at(self, aabb: Aabb3d) -> float | None:
        """Distance at which the swept AABB first hits `aabb`, or None."""

    def intersects(self, volume: Aabb3d) -> bool:
        """True if the swept AABB hits `volume`."""

    @property
    def aabb(self) -> Aabb3d:
        """The AABB being cast."""

    @aabb.setter
    def aabb(self, value: Aabb3d) -> None: ...

    @property
    def ray(self) -> RayCast3d:
        """The ray along which the AABB is cast."""

    @ray.setter
    def ray(self, value: RayCast3d) -> None: ...

    def __copy__(self) -> AabbCast3d:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> AabbCast3d:
        """Return an independent copy; the value holds no Python objects."""

class BoundingSphereCast:
    """A 3D sphere swept along a ray for collision testing."""

    def __init__(
        self, sphere: BoundingSphere, origin: Vec3A | Vec3, direction: Dir3, max: float
    ) -> None:
        """Cast a `BoundingSphere` from an origin along a direction for at most `max` distance."""

    @staticmethod
    def from_ray(sphere: BoundingSphere, ray: Ray3d, max: float) -> BoundingSphereCast:
        """Cast a `BoundingSphere` along a `Ray3d` for at most `max` distance."""

    def sphere_collision_at(self, sphere: BoundingSphere) -> float | None:
        """Distance at which the swept sphere first hits `sphere`, or None."""

    def intersects(self, volume: BoundingSphere) -> bool:
        """True if the swept sphere hits `volume`."""

    @property
    def sphere(self) -> BoundingSphere:
        """The sphere being cast."""

    @sphere.setter
    def sphere(self, value: BoundingSphere) -> None: ...

    @property
    def ray(self) -> RayCast3d:
        """The ray along which the sphere is cast."""

    @ray.setter
    def ray(self, value: RayCast3d) -> None: ...

    def __copy__(self) -> BoundingSphereCast:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> BoundingSphereCast:
        """Return an independent copy; the value holds no Python objects."""
