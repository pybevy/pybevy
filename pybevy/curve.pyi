from collections.abc import Sequence
from typing import ClassVar, Literal

from pybevy.math import Vec2, Vec3

class Interval:
    """A nonempty closed interval, possibly unbounded in either direction."""

    @property
    def start(self) -> float:
        """The start endpoint."""

    @property
    def end(self) -> float:
        """The end endpoint."""

    UNIT: ClassVar[Interval]
    EVERYWHERE: ClassVar[Interval]

    def __init__(self, start: float, end: float) -> None:
        """Create an interval with the given start and end.

        Raises:
            ValueError: if the interval would be empty or have a NaN endpoint.
        """

    def intersect(self, other: Interval) -> Interval:
        """Get the intersection of this interval with another.

        Raises:
            ValueError: if the resulting interval would be empty.
        """

    def length(self) -> float:
        """Get the length of the interval."""

    def is_bounded(self) -> bool:
        """Check if both endpoints are finite."""

    def has_finite_start(self) -> bool:
        """Check if the start endpoint is finite."""

    def has_finite_end(self) -> bool:
        """Check if the end endpoint is finite."""

    def contains(self, item: float) -> bool:
        """Check if a value is inside the interval."""

    def contains_interval(self, other: Interval) -> bool:
        """Check if another interval is fully contained in this one."""

    def clamp(self, value: float) -> float:
        """Clamp a value to the interval."""

    def spaced_points(self, points: int) -> list[float]:
        """Get exactly `points` evenly spaced values, including both endpoints.

        Zero returns an empty list and one returns only the start endpoint.
        At most 1,000,000 points.

        Raises:
            ValueError: if the interval is unbounded or `points` is outside
                [0, 1000000].
        """

    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __lt__(self, other: object) -> bool: ...
    def __le__(self, other: object) -> bool: ...
    def __gt__(self, other: object) -> bool: ...
    def __ge__(self, other: object) -> bool: ...

    def __copy__(self) -> Interval:
        """Return an independent copy of this value."""

    def __deepcopy__(self, memo: dict[int, object]) -> Interval:
        """Return an independent copy; the value holds no Python objects."""

class JumpAt:
    """Specifies where a jump should occur in a stepped easing function."""

    def __copy__(self) -> JumpAt: ...
    def __deepcopy__(self, memo: dict[int, object]) -> JumpAt: ...


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
    """Easing curves over the unit interval, as Bevy's `EaseFunction` curve.

    Every variant is a nested class; construct it before use.

    Example:
        >>> from pybevy.curve import EaseFunction, JumpAt
        >>> EaseFunction.Linear().sample_clamped(0.5)
        0.5
        >>> EaseFunction.QuadraticIn().sample(0.5)
        0.25
        >>> EaseFunction.QuadraticIn().sample(1.5) is None
        True
        >>> EaseFunction.Steps(4, JumpAt.End).sample_clamped(0.5)
        0.5
    """

    def domain(self) -> Interval:
        """The curve's domain, `Interval.UNIT`."""

    def sample(self, t: float) -> float | None:
        """Sample the curve at `t`, or return None when `t` lies outside the domain."""

    def sample_clamped(self, t: float) -> float:
        """Sample the curve at `t` after clamping `t` into the domain."""

    def __copy__(self) -> EaseFunction: ...
    def __deepcopy__(self, memo: dict[int, object]) -> EaseFunction: ...
    def __eq__(self, other: object) -> bool: ...

    class Linear(EaseFunction):
        """Linear interpolation (no easing)"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class QuadraticIn(EaseFunction):
        """Quadratic ease-in (slow start)"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class QuadraticOut(EaseFunction):
        """Quadratic ease-out (slow end)"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class QuadraticInOut(EaseFunction):
        """Quadratic ease-in-out (slow start and end)"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class CubicIn(EaseFunction):
        """Cubic ease-in"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class CubicOut(EaseFunction):
        """Cubic ease-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class CubicInOut(EaseFunction):
        """Cubic ease-in-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class QuarticIn(EaseFunction):
        """Quartic ease-in"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class QuarticOut(EaseFunction):
        """Quartic ease-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class QuarticInOut(EaseFunction):
        """Quartic ease-in-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class QuinticIn(EaseFunction):
        """Quintic ease-in"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class QuinticOut(EaseFunction):
        """Quintic ease-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class QuinticInOut(EaseFunction):
        """Quintic ease-in-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SmoothStepIn(EaseFunction):
        """Smooth step ease-in"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SmoothStepOut(EaseFunction):
        """Smooth step ease-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SmoothStep(EaseFunction):
        """Smooth step function"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SmootherStepIn(EaseFunction):
        """Smoother step ease-in"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SmootherStepOut(EaseFunction):
        """Smoother step ease-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SmootherStep(EaseFunction):
        """Smoother step function (5th order)"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SineIn(EaseFunction):
        """Sine ease-in"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SineOut(EaseFunction):
        """Sine ease-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SineInOut(EaseFunction):
        """Sine ease-in-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class CircularIn(EaseFunction):
        """Circular ease-in"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class CircularOut(EaseFunction):
        """Circular ease-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class CircularInOut(EaseFunction):
        """Circular ease-in-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ExponentialIn(EaseFunction):
        """Exponential ease-in"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ExponentialOut(EaseFunction):
        """Exponential ease-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ExponentialInOut(EaseFunction):
        """Exponential ease-in-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ElasticIn(EaseFunction):
        """Elastic ease-in"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ElasticOut(EaseFunction):
        """Elastic ease-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ElasticInOut(EaseFunction):
        """Elastic ease-in-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BackIn(EaseFunction):
        """Back ease-in (overshoots then returns)"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BackOut(EaseFunction):
        """Back ease-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BackInOut(EaseFunction):
        """Back ease-in-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BounceIn(EaseFunction):
        """Bounce ease-in"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BounceOut(EaseFunction):
        """Bounce ease-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BounceInOut(EaseFunction):
        """Bounce ease-in-out"""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Steps(EaseFunction):
        """`steps` steps connecting the start and the end, jumping as `jump` selects."""

        __match_args__: ClassVar[tuple[Literal["steps"], Literal["jump"]]]
        @property
        def steps(self) -> int: ...
        @property
        def jump(self) -> JumpAt: ...
        def __init__(self, steps: int, jump: JumpAt) -> None: ...

    class Elastic(EaseFunction):
        """`f(omega, t) = 1 - (1 - t)^2 (2 sin(omega t) / omega + cos(omega t))`."""

        __match_args__: ClassVar[tuple[Literal["omega"]]]
        @property
        def omega(self) -> float: ...
        def __init__(self, omega: float) -> None: ...

class CubicCurve2d:
    """A 2D cubic curve composed of cubic segments.

    A cubic curve is a parametric curve defined by cubic polynomial segments.
    It can be used to represent smooth paths and animations.

    Example:
        >>> from pybevy.curve import CubicBezier2d
        >>> from pybevy.math import Vec2
        >>> # Create a Bezier curve and convert it to a CubicCurve2d
        >>> bezier = CubicBezier2d([
        ...     [Vec2(0.0, 0.0), Vec2(1.0, 2.0), Vec2(2.0, 3.0), Vec2(3.0, 0.0)]
        ... ])
        >>> curve = bezier.to_curve()
    """

    def position(self, t: float) -> Vec2:
        """Compute the position on the curve at parameter t.

        Args:
            t: Parameter value (0 to the number of segments)

        Returns:
            The position on the curve at t
        """

    def velocity(self, t: float) -> Vec2:
        """Compute the velocity (first derivative) on the curve at parameter t.

        Args:
            t: Parameter value (0 to the number of segments)

        Returns:
            The velocity vector at t
        """

    def acceleration(self, t: float) -> Vec2:
        """Compute the acceleration (second derivative) on the curve at parameter t.

        Args:
            t: Parameter value (0 to the number of segments)

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

    def segments(self) -> list[CubicSegment2d]:
        """Independent copies of the segments that make up this curve."""
    def domain(self) -> Interval:
        """The parameter interval the curve is defined over."""
    def sample(self, t: float) -> Vec2 | None:
        """The position at `t`, or None when `t` lies outside the domain."""
    def sample_clamped(self, t: float) -> Vec2:
        """The position at `t` after clamping `t` into the domain."""

class CubicBezier2d:
    """A 2D cubic Bezier curve.

    A Bezier curve is defined by control points. Each segment requires 4 control points.
    The curve passes through the first and last control points of each segment.

    Example:
        >>> from pybevy.curve import CubicBezier2d
        >>> from pybevy.math import Vec2
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
        >>> from pybevy.curve import CubicCardinalSpline2d
        >>> from pybevy.math import Vec2
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

    def to_curve_cyclic(self) -> CubicCurve2d:
        """Build a closed curve that loops back to the first point.

        Raises:
            ValueError: If there are too few control points.
        """

class CubicHermite2d:
    """A 2D cubic Hermite curve.

    A Hermite curve is defined by control points and explicit tangent vectors.
    The curve passes through all control points with the specified velocities.

    Example:
        >>> from pybevy.curve import CubicHermite2d
        >>> from pybevy.math import Vec2
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
            tangents: List of tangent vectors at each control point, one per
                control point

        Raises:
            ValueError: If the two lists are not the same length
        """

    def to_curve(self) -> CubicCurve2d:
        """Convert the Hermite curve to a CubicCurve2d for evaluation.

        Returns:
            A CubicCurve2d that can be sampled

        Raises:
            ValueError: If insufficient control points were provided
        """

    def to_curve_cyclic(self) -> CubicCurve2d:
        """Build a closed curve that loops back to the first point.

        Raises:
            ValueError: If there are too few control points.
        """

class CubicCurve3d:
    """A 3D cubic curve composed of cubic segments."""

    def position(self, t: float) -> Vec3:
        """Compute the position on the curve at parameter t."""

    def velocity(self, t: float) -> Vec3:
        """Compute the velocity on the curve at parameter t."""

    def acceleration(self, t: float) -> Vec3:
        """Compute the acceleration on the curve at parameter t."""

    def iter_positions(self, subdivisions: int) -> list[Vec3]:
        """Sample positions along the curve."""

    def iter_velocities(self, subdivisions: int) -> list[Vec3]:
        """Sample velocities along the curve."""

    def iter_accelerations(self, subdivisions: int) -> list[Vec3]:
        """Sample accelerations along the curve."""

    def segments(self) -> list[CubicSegment3d]:
        """Independent copies of the segments that make up this curve."""
    def domain(self) -> Interval:
        """The parameter interval the curve is defined over."""
    def sample(self, t: float) -> Vec3 | None:
        """The position at `t`, or None when `t` lies outside the domain."""
    def sample_clamped(self, t: float) -> Vec3:
        """The position at `t` after clamping `t` into the domain."""

class CubicBezier3d:
    """A 3D cubic Bezier curve defined by control point segments.

    Example:
        >>> bezier = CubicBezier3d([[Vec3(0, 0, 0), Vec3(1, 0, 0), Vec3(1, 1, 0), Vec3(0, 1, 0)]])
    """

    def __init__(self, control_points: list[list[Vec3]]) -> None: ...

    def to_curve(self) -> CubicCurve3d:
        """Convert to a `CubicCurve3d` for evaluation."""

class CubicCardinalSpline3d:
    """A 3D cubic cardinal spline.

    Example:
        >>> spline = CubicCardinalSpline3d(0.1, [Vec3(0, 0, 0), Vec3(1, 0, 0), Vec3(2, 1, 0)])
    """

    def __init__(self, tension: float, control_points: list[Vec3]) -> None: ...

    @staticmethod
    def new_catmull_rom(control_points: list[Vec3]) -> CubicCardinalSpline3d:
        """Create a Catmull-Rom spline through the points."""

    def to_curve(self) -> CubicCurve3d:
        """Convert to a `CubicCurve3d` for evaluation."""

    @property
    def tension(self) -> float:
        """The spline tension."""

    @tension.setter
    def tension(self, value: float) -> None: ...

    def to_curve_cyclic(self) -> CubicCurve3d:
        """Build a closed curve that loops back to the first point.

        Raises:
            ValueError: If there are too few control points.
        """

class CubicHermite3d:
    """A 3D cubic Hermite spline.

    Example:
        >>> spline = CubicHermite3d([Vec3(0, 0, 0), Vec3(1, 0, 0)], [Vec3(1, 0, 0), Vec3(0, 1, 0)])
    """

    def __init__(self, control_points: list[Vec3], tangents: list[Vec3]) -> None:
        """Create a new Hermite curve.

        Args:
            control_points: List of points the curve should pass through
            tangents: List of tangent vectors at each control point, one per
                control point

        Raises:
            ValueError: If the two lists are not the same length
        """

    def to_curve(self) -> CubicCurve3d:
        """Convert to a `CubicCurve3d` for evaluation."""

    def to_curve_cyclic(self) -> CubicCurve3d:
        """Build a closed curve that loops back to the first point.

        Raises:
            ValueError: If there are too few control points.
        """

class CubicBSpline2d:
    """A 2D cubic B-spline.

    Example:
        >>> spline = CubicBSpline2d([Vec2(0, 0), Vec2(1, 0), Vec2(2, 1), Vec2(3, 0)])
    """

    def __init__(self, control_points: list[Vec2]) -> None: ...

    def to_curve(self) -> CubicCurve2d:
        """Convert to a `CubicCurve2d` for evaluation."""

    def to_curve_cyclic(self) -> CubicCurve2d:
        """Build a closed curve that loops back to the first point.

        Raises:
            ValueError: If there are too few control points.
        """

class CubicBSpline3d:
    """A 3D cubic B-spline.

    Example:
        >>> spline = CubicBSpline3d([Vec3(0, 0, 0), Vec3(1, 0, 0), Vec3(2, 1, 0), Vec3(3, 0, 0)])
    """

    def __init__(self, control_points: list[Vec3]) -> None: ...

    def to_curve(self) -> CubicCurve3d:
        """Convert to a `CubicCurve3d` for evaluation."""

    def to_curve_cyclic(self) -> CubicCurve3d:
        """Build a closed curve that loops back to the first point.

        Raises:
            ValueError: If there are too few control points.
        """

class LinearSpline2d:
    """A 2D linear spline.

    Example:
        >>> spline = LinearSpline2d([Vec2(0, 0), Vec2(1, 1), Vec2(2, 0)])
    """

    def __init__(self, points: list[Vec2]) -> None: ...

    def to_curve(self) -> CubicCurve2d:
        """Convert to a `CubicCurve2d` for evaluation."""

    def to_curve_cyclic(self) -> CubicCurve2d:
        """Build a closed curve that loops back to the first point.

        Raises:
            ValueError: If there are too few control points.
        """

class LinearSpline3d:
    """A 3D linear spline.

    Example:
        >>> spline = LinearSpline3d([Vec3(0, 0, 0), Vec3(1, 1, 1), Vec3(2, 0, 0)])
    """

    def __init__(self, points: list[Vec3]) -> None: ...

    def to_curve(self) -> CubicCurve3d:
        """Convert to a `CubicCurve3d` for evaluation."""

    def to_curve_cyclic(self) -> CubicCurve3d:
        """Build a closed curve that loops back to the first point.

        Raises:
            ValueError: If there are too few control points.
        """

class CubicNurbs2d:
    """A 2D non-uniform rational B-spline (NURBS) curve.

    Example:
        >>> nurbs = CubicNurbs2d([Vec2(0, 0), Vec2(1, 0), Vec2(2, 1), Vec2(3, 0)])
    """

    def __init__(
        self,
        control_points: list[Vec2],
        weights: list[float] | None = None,
        knots: list[float] | None = None,
    ) -> None:
        """Create a NURBS curve.

        Raises:
            ValueError: if fewer than 4 control points, wrong knot/weight counts,
                descending knots, or constant knots.
        """

    def to_curve(self) -> RationalCurve2d:
        """Convert to a `RationalCurve2d` for evaluation."""

    @property
    def control_points(self) -> list[Vec2]:
        """The control points."""

    @property
    def weights(self) -> list[float]:
        """The per-point weights."""

    @property
    def knots(self) -> list[float]:
        """The knot vector."""

class CubicNurbs3d:
    """A 3D non-uniform rational B-spline (NURBS) curve.

    Example:
        >>> nurbs = CubicNurbs3d([Vec3(0, 0, 0), Vec3(1, 0, 0), Vec3(2, 1, 0), Vec3(3, 0, 0)])
    """

    def __init__(
        self,
        control_points: list[Vec3],
        weights: list[float] | None = None,
        knots: list[float] | None = None,
    ) -> None:
        """Create a NURBS curve.

        Raises:
            ValueError: if fewer than 4 control points, wrong knot/weight counts,
                descending knots, or constant knots.
        """

    def to_curve(self) -> RationalCurve3d:
        """Convert to a `RationalCurve3d` for evaluation."""

    @property
    def control_points(self) -> list[Vec3]:
        """The control points."""

    @property
    def weights(self) -> list[float]:
        """The per-point weights."""

    @property
    def knots(self) -> list[float]:
        """The knot vector."""

class RationalCurve2d:
    """A 2D rational curve produced by `CubicNurbs2d.to_curve`."""

    def position(self, t: float) -> Vec2:
        """Compute the position on the curve at parameter t."""

    def velocity(self, t: float) -> Vec2:
        """Compute the velocity on the curve at parameter t."""

    def acceleration(self, t: float) -> Vec2:
        """Compute the acceleration on the curve at parameter t."""

    def segments(self) -> list[RationalSegment2d]:
        """Independent copies of the segments that make up this curve."""
    def domain(self) -> Interval:
        """The parameter interval the curve is defined over."""
    def sample(self, t: float) -> Vec2 | None:
        """The position at `t`, or None when `t` lies outside the domain."""
    def sample_clamped(self, t: float) -> Vec2:
        """The position at `t` after clamping `t` into the domain."""

class RationalCurve3d:
    """A 3D rational curve produced by `CubicNurbs3d.to_curve`."""

    def position(self, t: float) -> Vec3:
        """Compute the position on the curve at parameter t."""

    def velocity(self, t: float) -> Vec3:
        """Compute the velocity on the curve at parameter t."""

    def acceleration(self, t: float) -> Vec3:
        """Compute the acceleration on the curve at parameter t."""

    def segments(self) -> list[RationalSegment3d]:
        """Independent copies of the segments that make up this curve."""
    def domain(self) -> Interval:
        """The parameter interval the curve is defined over."""
    def sample(self, t: float) -> Vec3 | None:
        """The position at `t`, or None when `t` lies outside the domain."""
    def sample_clamped(self, t: float) -> Vec3:
        """The position at `t` after clamping `t` into the domain."""

class CubicSegment2d:
    """One polynomial segment of a cubic curve, with domain `[0, 1]`."""

    def __init__(self, *, coeff: list[Vec2] = ...) -> None:
        """Create a segment from its four polynomial coefficients (default all zero)."""
    @staticmethod
    def new_bezier(points: list[Vec2]) -> CubicSegment2d:
        """The segment of the Bezier curve through four control points."""
    @property
    def coeff(self) -> list[Vec2]:
        """Read-only snapshots of the coefficients; assign a list to replace them."""
    @coeff.setter
    def coeff(self, value: list[Vec2]) -> None: ...
    def position(self, t: float) -> Vec2: ...
    def velocity(self, t: float) -> Vec2: ...
    def acceleration(self, t: float) -> Vec2: ...
    def domain(self) -> Interval: ...
    def sample(self, t: float) -> Vec2 | None: ...
    def sample_clamped(self, t: float) -> Vec2: ...
    def __eq__(self, other: object) -> bool: ...
    def __copy__(self) -> CubicSegment2d: ...
    def __deepcopy__(self, memo: dict[int, object]) -> CubicSegment2d: ...

class RationalSegment2d:
    """One rational segment of a NURBS curve, with domain `[0, 1]`."""

    def __init__(
        self,
        *,
        coeff: list[Vec2] = ...,
        weight_coeff: list[float] = ...,
        knot_span: float = 0.0,
    ) -> None:
        """Create a segment from its coefficients, weights, and knot span (default all zero)."""
    @property
    def coeff(self) -> list[Vec2]:
        """Read-only snapshots of the coefficients; assign a list to replace them."""
    @coeff.setter
    def coeff(self, value: list[Vec2]) -> None: ...
    weight_coeff: list[float]
    knot_span: float
    def position(self, t: float) -> Vec2: ...
    def velocity(self, t: float) -> Vec2: ...
    def acceleration(self, t: float) -> Vec2: ...
    def domain(self) -> Interval: ...
    def sample(self, t: float) -> Vec2 | None: ...
    def sample_clamped(self, t: float) -> Vec2: ...
    def __eq__(self, other: object) -> bool: ...
    def __copy__(self) -> RationalSegment2d: ...
    def __deepcopy__(self, memo: dict[int, object]) -> RationalSegment2d: ...

class CubicSegment3d:
    """One polynomial segment of a cubic curve, with domain `[0, 1]`."""

    def __init__(self, *, coeff: list[Vec3] = ...) -> None:
        """Create a segment from its four polynomial coefficients (default all zero)."""
    @staticmethod
    def new_bezier(points: list[Vec3]) -> CubicSegment3d:
        """The segment of the Bezier curve through four control points."""
    @property
    def coeff(self) -> list[Vec3]:
        """Read-only snapshots of the coefficients; assign a list to replace them."""
    @coeff.setter
    def coeff(self, value: list[Vec3]) -> None: ...
    def position(self, t: float) -> Vec3: ...
    def velocity(self, t: float) -> Vec3: ...
    def acceleration(self, t: float) -> Vec3: ...
    def domain(self) -> Interval: ...
    def sample(self, t: float) -> Vec3 | None: ...
    def sample_clamped(self, t: float) -> Vec3: ...
    def __eq__(self, other: object) -> bool: ...
    def __copy__(self) -> CubicSegment3d: ...
    def __deepcopy__(self, memo: dict[int, object]) -> CubicSegment3d: ...

class RationalSegment3d:
    """One rational segment of a NURBS curve, with domain `[0, 1]`."""

    def __init__(
        self,
        *,
        coeff: list[Vec3] = ...,
        weight_coeff: list[float] = ...,
        knot_span: float = 0.0,
    ) -> None:
        """Create a segment from its coefficients, weights, and knot span (default all zero)."""
    @property
    def coeff(self) -> list[Vec3]:
        """Read-only snapshots of the coefficients; assign a list to replace them."""
    @coeff.setter
    def coeff(self, value: list[Vec3]) -> None: ...
    weight_coeff: list[float]
    knot_span: float
    def position(self, t: float) -> Vec3: ...
    def velocity(self, t: float) -> Vec3: ...
    def acceleration(self, t: float) -> Vec3: ...
    def domain(self) -> Interval: ...
    def sample(self, t: float) -> Vec3 | None: ...
    def sample_clamped(self, t: float) -> Vec3: ...
    def __eq__(self, other: object) -> bool: ...
    def __copy__(self) -> RationalSegment3d: ...
    def __deepcopy__(self, memo: dict[int, object]) -> RationalSegment3d: ...
