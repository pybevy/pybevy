from typing import ClassVar, Literal, TypeVar, overload

from pybevy.app import App, Plugin
from pybevy.camera import RenderLayers
from pybevy.color import Color
from pybevy.ecs import Component, Resource
from pybevy.math import Aabb3d, Isometry2d, Isometry3d, Vec2, Vec3

class GizmoLineJoint:
    def __hash__(self) -> int: ...

    class None_(GizmoLineJoint):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Miter(GizmoLineJoint):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Round(GizmoLineJoint):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    class Bevel(GizmoLineJoint):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

class GizmoLineStyle:
    def __hash__(self) -> int: ...

    class Solid(GizmoLineStyle):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Dotted(GizmoLineStyle):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Dashed(GizmoLineStyle):
        __match_args__: ClassVar[tuple[Literal["gap_scale"], Literal["line_scale"]]]
        gap_scale: float
        line_scale: float
        def __init__(self, gap_scale: float, line_scale: float) -> None: ...

class GizmoLineConfig:
    def __init__(
        self,
        width: float = 2.0,
        perspective: bool = False,
        style: GizmoLineStyle = GizmoLineStyle.Solid(),
        joints: GizmoLineJoint = GizmoLineJoint.None_(),
    ) -> None: ...
    @property
    def width(self) -> float: ...
    @width.setter
    def width(self, value: float) -> None: ...
    @property
    def perspective(self) -> bool: ...
    @perspective.setter
    def perspective(self, value: bool) -> None: ...
    @property
    def style(self) -> GizmoLineStyle: ...
    @style.setter
    def style(self, value: GizmoLineStyle) -> None: ...
    @property
    def joints(self) -> GizmoLineJoint: ...
    @joints.setter
    def joints(self, value: GizmoLineJoint) -> None: ...

class GizmoConfig:
    def __init__(
        self,
        enabled: bool = True,
        line: GizmoLineConfig = GizmoLineConfig(),
        depth_bias: float = 0.0,
        render_layers: RenderLayers | None = None,
    ) -> None: ...
    @property
    def enabled(self) -> bool: ...
    @enabled.setter
    def enabled(self, value: bool) -> None: ...
    @property
    def line(self) -> GizmoLineConfig: ...
    @line.setter
    def line(self, value: GizmoLineConfig) -> None: ...
    @property
    def depth_bias(self) -> float: ...
    @depth_bias.setter
    def depth_bias(self, value: float) -> None: ...
    @property
    def render_layers(self) -> RenderLayers: ...
    @render_layers.setter
    def render_layers(self, value: RenderLayers) -> None: ...

class GizmoConfigGroup:
    """Base marker for native Bevy gizmo configuration groups."""

_GizmoConfigGroupT = TypeVar("_GizmoConfigGroupT", bound=GizmoConfigGroup)

class GizmoConfigStore(Resource):
    """Configuration store for default and registered native gizmo groups.

    Calls without a group return the default group's common configuration.
    Calls with a group class return ``(common_config, typed_group_config)``.
    """

    @overload
    def config(self, group: None = None) -> GizmoConfig: ...
    @overload
    def config(
        self, group: type[_GizmoConfigGroupT]
    ) -> tuple[GizmoConfig, _GizmoConfigGroupT]: ...
    @overload
    def config_mut(self, group: None = None) -> GizmoConfig: ...
    @overload
    def config_mut(
        self, group: type[_GizmoConfigGroupT]
    ) -> tuple[GizmoConfig, _GizmoConfigGroupT]: ...

class Gizmos:
    """Immediate-mode debug drawing system parameter."""

    @property
    def config(self) -> GizmoConfig:
        """Read-only borrowed configuration for this system invocation."""

    def line(self, start: Vec3, end: Vec3, color: Color) -> None: ...
    def line_gradient(
        self, start: Vec3, end: Vec3, start_color: Color, end_color: Color
    ) -> None: ...
    def ray(self, start: Vec3, vector: Vec3, color: Color) -> None: ...
    def ray_gradient(
        self, start: Vec3, vector: Vec3, start_color: Color, end_color: Color
    ) -> None: ...
    def linestrip(self, positions: list[Vec3], color: Color) -> None: ...
    def linestrip_gradient(self, points: list[tuple[Vec3, Color]]) -> None: ...
    def lineloop(self, positions: list[Vec3], color: Color) -> None: ...
    def line_2d(self, start: Vec2, end: Vec2, color: Color) -> None: ...
    def line_gradient_2d(
        self, start: Vec2, end: Vec2, start_color: Color, end_color: Color
    ) -> None: ...
    def ray_2d(self, start: Vec2, vector: Vec2, color: Color) -> None: ...
    def ray_gradient_2d(
        self, start: Vec2, vector: Vec2, start_color: Color, end_color: Color
    ) -> None: ...
    def linestrip_2d(self, positions: list[Vec2], color: Color) -> None: ...
    def linestrip_gradient_2d(self, points: list[tuple[Vec2, Color]]) -> None: ...
    def lineloop_2d(self, positions: list[Vec2], color: Color) -> None: ...
    def rect(self, isometry: Isometry3d, size: Vec2, color: Color) -> None: ...
    def cube(self, transform: Isometry3d, color: Color) -> None: ...
    def aabb_3d(
        self, aabb: Aabb3d, transform: Isometry3d, color: Color
    ) -> None: ...
    def rect_2d(self, isometry: Isometry2d, size: Vec2, color: Color) -> None: ...
    def cross(
        self, isometry: Isometry3d, half_size: float, color: Color
    ) -> None: ...
    def cross_2d(
        self, isometry: Isometry2d, half_size: float, color: Color
    ) -> None: ...
    def ellipse(
        self,
        isometry: Isometry3d,
        half_size: Vec2,
        color: Color,
        resolution: int = 32,
    ) -> None: ...
    def ellipse_2d(
        self,
        isometry: Isometry2d,
        half_size: Vec2,
        color: Color,
        resolution: int = 32,
    ) -> None: ...
    def circle(
        self,
        isometry: Isometry3d,
        radius: float,
        color: Color,
        resolution: int = 32,
    ) -> None: ...
    def circle_2d(
        self,
        isometry: Isometry2d,
        radius: float,
        color: Color,
        resolution: int = 32,
    ) -> None: ...
    def sphere(
        self,
        isometry: Isometry3d,
        radius: float,
        color: Color,
        resolution: int = 32,
    ) -> None: ...
    def arrow(
        self,
        start: Vec3,
        end: Vec3,
        color: Color,
        *,
        tip_length: float | None = None,
        double_ended: bool = False,
    ) -> None: ...
    def arrow_2d(
        self,
        start: Vec2,
        end: Vec2,
        color: Color,
        *,
        tip_length: float | None = None,
        double_ended: bool = False,
    ) -> None: ...

class ShowAabbGizmo(Component):
    """Draw the entity's axis-aligned bounding box."""

    def __init__(self, color: Color | None = None) -> None: ...
    color: Color | None

class GizmoPlugin(Plugin):
    """Gizmo plugin; requires AssetPlugin and MeshPlugin before it."""

    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...
