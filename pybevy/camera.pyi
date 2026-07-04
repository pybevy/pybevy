from typing import ClassVar

from pybevy.app import App, Plugin
from pybevy.assets import Handle
from pybevy.color import Color
from pybevy.ecs import Batchable, Component, Entity, Resource
from pybevy.image import Image
from pybevy.math import (
    Affine3A,
    Mat3A,
    Mat4,
    Quat,
    Range,
    Ray3d,
    Rect,
    URect,
    UVec2,
    Vec2,
    Vec3,
    Vec3A,
    Vec4,
)
from pybevy.transform import GlobalTransform

try:
    import numpy as np
except ImportError:
    pass

class CameraPlugin(Plugin):
    """Camera plugin providing camera projection and rendering systems."""
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class ScalingMode:
    """Camera scaling mode for orthographic projections."""

    @staticmethod
    def WindowSize() -> ScalingMode:
        """Match the viewport size. With scale=1, world units map 1:1 to pixels."""

    @staticmethod
    def Fixed(width: float, height: float) -> ScalingMode:
        """Manually specify projection size, ignoring window resizing."""

    @staticmethod
    def AutoMin(min_width: float, min_height: float) -> ScalingMode:
        """Keep aspect ratio while axes can't be smaller than given minimum."""

    @staticmethod
    def AutoMax(max_width: float, max_height: float) -> ScalingMode:
        """Keep aspect ratio while axes can't be bigger than given maximum."""

    @staticmethod
    def FixedVertical(viewport_height: float) -> ScalingMode:
        """Keep projection height constant; width adjusts to match aspect ratio."""

    @staticmethod
    def FixedHorizontal(viewport_width: float) -> ScalingMode:
        """Keep projection width constant; height adjusts to match aspect ratio."""

class OrthographicProjection:
    """Orthographic camera projection for isometric and 2D games."""

    near: float
    far: float
    viewport_origin: Vec2
    scaling_mode: ScalingMode
    scale: float
    area: Rect

    def __init__(
        self,
        near: float = 0.0,
        far: float = 1000.0,
        viewport_origin: Vec2 = ...,
        scaling_mode: ScalingMode = ...,
        scale: float = 1.0,
        area: Rect = ...,
    ) -> None: ...

    @staticmethod
    def default_3d() -> OrthographicProjection:
        """Returns the default orthographic projection for a 3D context."""

    @staticmethod
    def default_2d() -> OrthographicProjection:
        """Returns the default orthographic projection for a 2D context."""

    def get_clip_from_view(self) -> Mat4:
        """Generate the projection matrix.

        Returns:
            The clip-from-view (projection) matrix
        """

    def update(self, width: float, height: float) -> None:
        """Update projection for a new render area size.

        Updates the projection area based on the scaling mode and dimensions.

        Args:
            width: New width of the render area
            height: New height of the render area
        """

    def get_frustum_corners(self, z_near: float, z_far: float) -> list[Vec3A]:
        """Get the eight corners of the camera frustum.

        Args:
            z_near: Near plane distance
            z_far: Far plane distance

        Returns:
            List of 8 Vec3A corners in the order: bottom right, top right,
            top left, bottom left for near plane, then same for far plane.
        """

    def compute_frustum(self, camera_transform: GlobalTransform) -> Frustum:
        """Compute camera frustum from this projection and transform.

        Args:
            camera_transform: The camera's global transform

        Returns:
            The computed frustum for visibility culling
        """

    def get_clip_from_view_for_sub(self, sub_view: SubCameraView) -> Mat4:
        """Generate the projection matrix for a SubCameraView.

        Used when rendering a sub-region of the full camera view.

        Args:
            sub_view: The sub-camera view configuration

        Returns:
            The clip-from-view (projection) matrix adjusted for the sub-view
        """

class Camera2d(Component):
    def __init__(self) -> None: ...
    def __copy__(self) -> Camera2d: ...
    def __deepcopy__(self, memo: dict[int, object]) -> Camera2d: ...

class Camera3dDepthLoadOp:
    """Depth load operation for 3D cameras."""
    @staticmethod
    def Clear(value: float) -> Camera3dDepthLoadOp: ...
    @staticmethod
    def Load() -> Camera3dDepthLoadOp: ...

class Camera3dDepthTextureUsage:
    """Texture usage flags for depth buffer."""
    def __init__(self, flags: int) -> None: ...

class ScreenSpaceTransmissionQuality:
    """Quality setting for screen space specular transmission."""
    Low: ScreenSpaceTransmissionQuality
    Medium: ScreenSpaceTransmissionQuality
    High: ScreenSpaceTransmissionQuality
    Ultra: ScreenSpaceTransmissionQuality

class Camera3d(Component):
    """3D camera component with configurable depth and transmission settings."""

    depth_load_op: Camera3dDepthLoadOp
    depth_texture_usages: Camera3dDepthTextureUsage
    screen_space_specular_transmission_steps: int
    screen_space_specular_transmission_quality: ScreenSpaceTransmissionQuality

    def __init__(
        self,
        depth_load_op: Camera3dDepthLoadOp = ...,
        depth_texture_usages: Camera3dDepthTextureUsage = ...,
        screen_space_specular_transmission_steps: int = 1,
        screen_space_specular_transmission_quality: ScreenSpaceTransmissionQuality = ...,
    ) -> None: ...
    def __copy__(self) -> Camera3d: ...
    def __deepcopy__(self, memo: dict[int, object]) -> Camera3d: ...

class RenderTarget(Component):
    """Specifies where a camera renders to.

    This is a required component of Camera — it is automatically added when Camera
    is spawned, defaulting to the primary window. You can override it by including
    RenderTarget in the spawn tuple.

    Can be:
    - Window: Render to the primary window surface
    - Image: Render to a texture/image asset (for headless rendering, screenshots, etc.)
    - TextureView: Render to a manually managed texture view
    - None: No rendering output, just run the pipeline (useful for depth prepass)

    Examples:
        >>> # Render to window (default)
        >>> target = RenderTarget.window()
        >>>
        >>> # Render to texture for headless rendering
        >>> image = Image.new_render_target(width=800, height=600)
        >>> handle = images.add(image)
        >>> commands.spawn((Camera3d(), Camera(), RenderTarget.image(handle)))
        >>>
        >>> # No rendering (depth prepass only)
        >>> target = RenderTarget.none(UVec2(800, 600))
    """

    @staticmethod
    def window() -> RenderTarget:
        """Create a RenderTarget that renders to the primary window.

        Returns:
            RenderTarget configured for window rendering
        """

    @staticmethod
    def image(handle: Handle[Image]) -> RenderTarget:
        """Create a RenderTarget that renders to an image texture.

        The image must be created with appropriate texture usage flags:
        - RENDER_ATTACHMENT: Can be used as a render target
        - COPY_SRC: Can be copied from (for readback)
        - TEXTURE_BINDING: Can be sampled in shaders

        Use `Image.new_render_target()` to create a properly configured image.

        Args:
            handle: Handle to an Image asset with render target usage flags

        Returns:
            RenderTarget configured for image rendering

        Examples:
            >>> image = Image.new_render_target(width=800, height=600)
            >>> handle = assets.add(image)
            >>> target = RenderTarget.image(handle)
        """

    @staticmethod
    def texture_view(texture_view_id: int) -> RenderTarget:
        """Create a RenderTarget that renders to a manually managed texture view.

        Used for advanced rendering scenarios where you manage the texture view
        lifecycle manually via the render app.

        Args:
            texture_view_id: The ManualTextureViewHandle ID (u32)

        Returns:
            RenderTarget configured for manual texture view rendering
        """

    @staticmethod
    def none(size: UVec2) -> RenderTarget:
        """Create a RenderTarget with no output, just running the render pipeline.

        Useful for depth prepass cameras that need to run the pipeline and
        build depth buffers without producing any color output.

        Args:
            size: The logical size for the "virtual" render target

        Returns:
            RenderTarget configured for no-output rendering
        """

    def as_image(self) -> Handle[Image] | None:
        """Get the image handle if this is an image render target.

        Returns:
            The image handle if this is RenderTarget.image(...), None otherwise
        """

    def normalize(
        self, primary_window: Entity | None = None
    ) -> NormalizedRenderTarget | None:
        """Normalize the render target for equality comparisons.

        Converts window references to their normalized form using the
        primary window if available.

        Args:
            primary_window: Optional primary window entity for normalization

        Returns:
            Normalized render target, or None if normalization fails
        """

class Viewport(Component):
    """Viewport configuration for rendering to a specific region.

    Defines the rectangular region within the render target where the camera
    will render. Useful for split-screen, mini-maps, picture-in-picture, and
    UI overlays.

    Attributes:
        physical_position: Position in pixels from top-left corner of render target
        physical_size: Size in pixels of the viewport rectangle
        depth: Depth range (near, far) as a tuple, typically (0.0, 1.0)

    Examples:
        >>> # Full-screen viewport
        >>> viewport = Viewport(
        ...     physical_position=UVec2(0, 0),
        ...     physical_size=UVec2(1920, 1080)
        ... )
        >>>
        >>> # Split-screen left half
        >>> left = Viewport(
        ...     physical_position=UVec2(0, 0),
        ...     physical_size=UVec2(960, 1080)
        ... )
        >>>
        >>> # Mini-map in top-right corner
        >>> minimap = Viewport(
        ...     physical_position=UVec2(1720, 0),
        ...     physical_size=UVec2(200, 200),
        ...     depth=(0.0, 1.0)
        ... )
    """

    def __init__(
        self,
        physical_position: UVec2,
        physical_size: UVec2,
        depth: tuple[float, float] = (0.0, 1.0),
    ) -> None:
        """Create a new Viewport.

        Args:
            physical_position: Position in pixels from top-left of render target
            physical_size: Size in pixels of the viewport
            depth: Tuple of (near, far) depth range, defaults to (0.0, 1.0)
        """

    @property
    def physical_position(self) -> UVec2:
        """Get the viewport's physical position in pixels from top-left.

        Returns:
            Position in pixels
        """

    @physical_position.setter
    def physical_position(self, value: UVec2) -> None:
        """Set the viewport's physical position.

        Args:
            value: Position in pixels from top-left of render target
        """

    @property
    def physical_size(self) -> UVec2:
        """Get the viewport's physical size in pixels.

        Returns:
            Size in pixels
        """

    @physical_size.setter
    def physical_size(self, value: UVec2) -> None:
        """Set the viewport's physical size.

        Args:
            value: Size in pixels
        """

    @property
    def depth(self) -> tuple[float, float]:
        """Get the viewport's depth range as (near, far) tuple.

        The depth range controls which parts of the scene are rendered based
        on distance from the camera. Objects outside this range are clipped.

        Returns:
            (near, far) depth range, typically (0.0, 1.0)
        """

    @depth.setter
    def depth(self, value: tuple[float, float]) -> None:
        """Set the viewport's depth range.

        Args:
            value: Tuple of (near, far) depth range
        """

    def __copy__(self) -> Viewport: ...
    def __deepcopy__(self, memo: dict[int, object]) -> Viewport: ...

    def clamp_to_size(self, size: UVec2) -> None:
        """Clamp the viewport rectangle to fit within the given size.

        If the viewport position is outside the size, it will be adjusted.
        If the viewport extends beyond the size, it will be truncated.

        Args:
            size: Maximum size to clamp to
        """

    @staticmethod
    def from_viewport_and_override(
        viewport: Viewport | None,
        main_pass_resolution_override: UVec2 | None,
    ) -> Viewport | None:
        """Create a viewport from an optional viewport and resolution override.

        If main_pass_resolution_override is provided, it will override the
        viewport's physical_size (creating a default viewport if none provided).

        Args:
            viewport: Optional existing viewport
            main_pass_resolution_override: Optional size override

        Returns:
            The resulting viewport, or None if neither input is provided
        """

class Camera(Component):
    """Camera component that configures rendering.

    Separate from Camera2d/Camera3d marker components. Contains the actual
    rendering configuration including the order, active state, and optional
    viewport. Use the RenderTarget component separately to control where
    the camera renders.

    Examples:
        >>> # Create camera with default settings (renders to window)
        >>> camera = Camera()
        >>>
        >>> # Configure render order (lower renders first)
        >>> camera.order = -1
        >>>
        >>> # Disable camera
        >>> camera.is_active = False
        >>>
        >>> # Split-screen setup
        >>> camera.viewport = Viewport(
        ...     physical_position=UVec2(0, 0),
        ...     physical_size=UVec2(960, 1080)
        ... )
    """

    def __init__(
        self,
        is_active: bool = True,
        *,
        order: int = 0,
        msaa_writeback: str = "auto",
        clear_color: ClearColorConfig | None = None,
        viewport: Viewport | None = None,
        sub_camera_view: SubCameraView | None = None,
    ) -> None:
        """Create a new Camera with optional configuration.

        Args:
            is_active: Whether the camera should be active
            order: Render order (lower renders first)
            msaa_writeback: MSAA writeback mode ("off", "auto", or "always")
            clear_color: How to clear the render target
            viewport: Optional viewport (subset of render target)
            sub_camera_view: Optional sub-camera view configuration
        """

    @property
    def is_active(self) -> bool:
        """Get whether the camera is active/enabled.

        Returns:
            True if camera is active, False if disabled
        """

    @is_active.setter
    def is_active(self, value: bool) -> None:
        """Set whether the camera is active.

        Args:
            value: True to enable camera, False to disable
        """

    @property
    def order(self) -> int:
        """Get the camera's render order.

        Cameras with lower order values are rendered first.

        Returns:
            Render order (default: 0)
        """

    @order.setter
    def order(self, value: int) -> None:
        """Set the camera's render order.

        Args:
            value: Render order (cameras with lower values render first)
        """

    @property
    def msaa_writeback(self) -> str:
        """Get the MSAA writeback mode ("off", "auto", or "always")."""

    @msaa_writeback.setter
    def msaa_writeback(self, value: str) -> None:
        """Set the MSAA writeback mode.

        Args:
            value: One of "off", "auto", or "always"
        """

    @property
    def invert_culling(self) -> bool:
        """Whether to invert culling for mirrored cameras."""

    @invert_culling.setter
    def invert_culling(self, value: bool) -> None: ...

    @property
    def clear_color(self) -> ClearColorConfig:
        """Get the camera's clear color configuration."""

    @clear_color.setter
    def clear_color(self, value: ClearColorConfig) -> None:
        """Set the camera's clear color configuration."""

    @property
    def viewport(self) -> Viewport | None:
        """Get the camera's viewport configuration.

        The viewport defines the rectangular region within the render target
        where the camera will render. None means render to the full target.

        Returns:
            Viewport configuration, or None for full render target

        Examples:
            >>> camera = Camera()
            >>> viewport = camera.viewport
            >>> if viewport:
            ...     print(f"Position: {viewport.physical_position}")
            ...     print(f"Size: {viewport.physical_size}")
        """

    @viewport.setter
    def viewport(self, value: Viewport | None) -> None:
        """Set the camera's viewport configuration.

        Args:
            value: Viewport to use, or None for full render target

        Examples:
            >>> # Split-screen left half
            >>> camera.viewport = Viewport(
            ...     physical_position=UVec2(0, 0),
            ...     physical_size=UVec2(960, 1080)
            ... )
            >>>
            >>> # Reset to full screen
            >>> camera.viewport = None
        """

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        is_active: np.ndarray | None = None,
    ) -> Batchable: ...

    def __copy__(self) -> Camera: ...
    def __deepcopy__(self, memo: dict[int, object]) -> Camera: ...

    def viewport_to_world_2d(
        self, camera_transform: GlobalTransform, viewport_position: Vec2
    ) -> Vec2:
        """Convert viewport position to 2D world position.

        This method converts a position in viewport coordinates (e.g., mouse cursor position)
        to world coordinates in 2D space. Useful for determining what position in the game world
        the user is pointing at.

        Args:
            camera_transform: The GlobalTransform of the camera entity
            viewport_position: Position in viewport coordinates (pixels from top-left)

        Returns:
            World position in 2D space

        Raises:
            ValueError: If viewport size is not available or conversion fails

        Example:
            >>> def mouse_to_world(
            ...     camera_query: Single[tuple[Camera, GlobalTransform], With[Camera2d]],
            ...     window: Single[Window],
            ... ) -> None:
            ...     camera, transform = camera_query.get()
            ...     if cursor_pos := window.cursor_position():
            ...         world_pos = camera.viewport_to_world_2d(transform, cursor_pos)
            ...         print(f"World position: {world_pos}")
        """

    def world_to_viewport(
        self, camera_transform: GlobalTransform, world_position: Vec3
    ) -> Vec2:
        """Convert world position to viewport position.

        This method converts a position in world coordinates to viewport coordinates.
        Useful for placing UI elements at world positions or checking if a world position
        is visible on screen.

        Args:
            camera_transform: The GlobalTransform of the camera entity
            world_position: Position in 3D world space

        Returns:
            Position in viewport coordinates (pixels from top-left)

        Raises:
            ValueError: If viewport size is not available or position is not visible

        Example:
            >>> def check_visibility(
            ...     camera_query: Single[tuple[Camera, GlobalTransform], With[Camera3d]],
            ...     entity_query: Query[GlobalTransform, With[Enemy]],
            ... ) -> None:
            ...     camera, cam_transform = camera_query.get()
            ...     for entity_transform in entity_query:
            ...         world_pos = entity_transform.translation
            ...         try:
            ...             screen_pos = camera.world_to_viewport(cam_transform, world_pos)
            ...             print(f"Enemy at screen position: {screen_pos}")
            ...         except ValueError:
            ...             pass  # Enemy not visible
        """

    def viewport_to_world(
        self, camera_transform: GlobalTransform, viewport_position: Vec2
    ) -> Ray3d:
        """Convert viewport position to a 3D ray in world space.

        This method converts a position in viewport coordinates (e.g., mouse cursor position)
        to a ray in world space. The ray originates at the camera position and passes through
        the viewport position in 3D space. Useful for raycasting/picking.

        Args:
            camera_transform: The GlobalTransform of the camera entity
            viewport_position: Position in viewport coordinates (pixels from top-left)

        Returns:
            Ray3d with origin at camera position and direction through viewport point

        Raises:
            ValueError: If viewport size is not available or conversion fails

        Example:
            >>> def raycast_from_cursor(
            ...     camera_query: Single[tuple[Camera, GlobalTransform], With[Camera3d]],
            ...     window: Single[Window],
            ... ) -> None:
            ...     camera, transform = camera_query.get()
            ...     if cursor_pos := window.cursor_position():
            ...         ray = camera.viewport_to_world(transform, cursor_pos)
            ...         # Use ray.origin and ray.direction for raycasting
        """

    def world_to_viewport_with_depth(
        self, camera_transform: GlobalTransform, world_position: Vec3
    ) -> Vec3:
        """Convert world position to viewport position with depth.

        Like world_to_viewport, but also returns the depth (Z) component,
        which represents the distance from the camera in normalized device coordinates.

        Args:
            camera_transform: The GlobalTransform of the camera entity
            world_position: Position in 3D world space

        Returns:
            Vec3 where x,y are viewport coordinates and z is the depth value

        Raises:
            ValueError: If viewport size is not available or conversion fails
        """

    @property
    def sub_camera_view(self) -> SubCameraView | None:
        """Get the sub-camera view configuration for tiled/split-screen rendering."""
    @sub_camera_view.setter
    def sub_camera_view(self, value: SubCameraView | None) -> None:
        """Set the sub-camera view configuration."""

    def to_logical(self, physical_size: UVec2) -> Vec2 | None:
        """Convert physical size to logical size using camera's scaling factor."""
    def physical_viewport_rect(self) -> URect | None:
        """Get the physical viewport rectangle in pixels."""
    def logical_viewport_rect(self) -> Rect | None:
        """Get the logical viewport rectangle."""
    def logical_viewport_size(self) -> Vec2 | None:
        """Get the logical viewport size."""
    def physical_viewport_size(self) -> UVec2 | None:
        """Get the physical viewport size in pixels."""
    def logical_target_size(self) -> Vec2 | None:
        """Get the logical target size."""
    def physical_target_size(self) -> UVec2 | None:
        """Get the physical target size in pixels."""
    def target_scaling_factor(self) -> float | None:
        """Get the scaling factor between physical and logical sizes."""

    def clip_from_view(self) -> Mat4:
        """Get the clip-from-view matrix computed for this camera.

        Returns:
            The clip-from-view (projection) matrix
        """

    def depth_ndc_to_view_z(self, ndc_depth: float) -> float:
        """Convert depth in Normalized Device Coordinates to view-space Z coordinate.

        For 3D cameras with reverse depth (infinite far plane).

        Args:
            ndc_depth: Depth value in NDC space

        Returns:
            View-space Z coordinate
        """

    def depth_ndc_to_view_z_2d(self, ndc_depth: float) -> float:
        """Convert depth in Normalized Device Coordinates to view-space Z coordinate.

        For 2D cameras with finite depth range.

        Args:
            ndc_depth: Depth value in NDC space

        Returns:
            View-space Z coordinate
        """

class Visibility(Component):
    INHERITED: ClassVar[Visibility]
    VISIBLE: ClassVar[Visibility]
    HIDDEN: ClassVar[Visibility]
    def __init__(self) -> None: ...
    @staticmethod
    def from_numpy(visibility: np.ndarray) -> Batchable: ...  # type: ignore[override]
    def toggle_inherited_visible(self) -> None:
        """Toggle between Inherited and Visible states."""
    def toggle_inherited_hidden(self) -> None:
        """Toggle between Inherited and Hidden states."""
    def toggle_visible_hidden(self) -> None:
        """Toggle between Visible and Hidden states."""
    def set_visible(self) -> None:
        """Set visibility to Visible."""
    def set_hidden(self) -> None:
        """Set visibility to Hidden."""
    def set_inherited(self) -> None:
        """Set visibility to Inherited."""

class VisibilityRange(Component):
    """Controls visibility based on camera distance for LOD (Level of Detail) systems.

    Entities with VisibilityRange are visible only when the camera distance falls
    within the specified margins. This enables automatic LOD transitions.

    Attributes:
        start_margin: Distance range where entity fades in (start..end)
        end_margin: Distance range where entity fades out (start..end)
        use_aabb: If True, use AABB center for distance calculation instead of entity origin
    """

    def __init__(
        self,
        start_margin: Range | None = None,
        end_margin: Range | None = None,
        use_aabb: bool = False,
    ) -> None: ...
    @staticmethod
    def abrupt(start: float, end: float) -> VisibilityRange:
        """Create visibility range with no crossfading.

        Args:
            start: Distance where entity becomes visible
            end: Distance where entity becomes hidden
        """

    @property
    def start_margin(self) -> Range: ...
    @start_margin.setter
    def start_margin(self, value: Range) -> None: ...

    @property
    def end_margin(self) -> Range: ...
    @end_margin.setter
    def end_margin(self, value: Range) -> None: ...

    @property
    def use_aabb(self) -> bool: ...
    @use_aabb.setter
    def use_aabb(self, value: bool) -> None: ...

    def is_abrupt(self) -> bool:
        """Returns True if this range has no crossfading."""
    def is_visible_at_all(self, camera_distance: float) -> bool:
        """Check if entity would be visible at this distance."""
    def is_culled(self, camera_distance: float) -> bool:
        """Check if entity would be culled at this distance."""

    def __eq__(self, other: object) -> bool:
        """Check equality with another VisibilityRange."""

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        use_aabb: np.ndarray | None = None,
    ) -> Batchable: ...

class NoFrustumCulling(Component):
    """Marker component that disables frustum culling for an entity.

    Entities with this component are always rendered regardless of whether
    they're inside the camera's view frustum. Useful for large entities
    that may be partially visible from unexpected angles.
    """

    def __init__(self) -> None: ...

class NoCpuCulling(Component):
    """Marker component that disables CPU-side visibility culling for a camera.

    When attached to a camera entity, disables CPU-side visibility culling
    entirely for that camera. This can be useful for performance testing
    or when culling is handled on the GPU.
    """

    def __init__(self) -> None: ...

class Sphere:
    """A bounding sphere used for frustum culling calculations.

    This is the camera primitive Sphere used with Frustum.intersects_sphere().
    Not to be confused with the math primitive Sphere.
    """

    @property
    def center(self) -> Vec3A: ...
    @center.setter
    def center(self, value: Vec3A) -> None: ...

    @property
    def radius(self) -> float: ...
    @radius.setter
    def radius(self, value: float) -> None: ...

    def __init__(
        self, center: Vec3A | None = None, radius: float = 0.0
    ) -> None: ...

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

class Aabb(Component):
    """An axis-aligned bounding box, defined by a center and half-extents.

    Used for frustum culling to determine if entities should be rendered.
    """

    @property
    def center(self) -> Vec3A: ...
    @center.setter
    def center(self, value: Vec3A) -> None: ...
    @property
    def half_extents(self) -> Vec3A: ...
    @half_extents.setter
    def half_extents(self, value: Vec3A) -> None: ...

    def __init__(
        self, center: Vec3A | None = None, half_extents: Vec3A | None = None
    ) -> None: ...

    @staticmethod
    def from_min_max(minimum: Vec3, maximum: Vec3) -> Aabb:
        """Creates an Aabb from minimum and maximum points."""

    @staticmethod
    def enclosing(iter: list[Vec3]) -> Aabb | None:
        """Returns a bounding box enclosing the specified points, or None if empty."""

    def min(self) -> Vec3A:
        """Returns the minimum point of the bounding box."""

    def max(self) -> Vec3A:
        """Returns the maximum point of the bounding box."""

    def relative_radius(self, p_normal: Vec3A, world_from_local: Mat3A) -> float:
        """Calculate the relative radius of the AABB with respect to a plane.

        Args:
            p_normal: The plane normal.
            world_from_local: Transform matrix from local to world space.

        Returns:
            The relative radius.
        """

    def is_in_half_space(self, half_space: HalfSpace, world_from_local: Affine3A) -> bool:
        """Check if the AABB is at the front side of the bisecting plane.

        Args:
            half_space: The half-space to test against.
            world_from_local: Affine transform from local to world space.

        Returns:
            True if the AABB is on the front side of the plane.
        """

    def is_in_half_space_identity(self, half_space: HalfSpace) -> bool:
        """Optimized half-space test for an AABB already in world space."""

class Frustum(Component):
    """A region of 3D space defined by the intersection of 6 half-spaces.

    Frustums are typically an apex-truncated square pyramid (a pyramid without the top) or a cuboid.
    Used for frustum culling to determine which entities should be rendered.
    """

    @property
    def half_spaces(self) -> list[HalfSpace]: ...
    @half_spaces.setter
    def half_spaces(self, value: list[HalfSpace]) -> None: ...

    def __init__(self) -> None: ...

    @staticmethod
    def from_clip_from_world(clip_from_world: Mat4) -> Frustum:
        """Creates a frustum from a clip-from-world matrix."""

    @staticmethod
    def from_clip_from_world_custom_far(
        clip_from_world: Mat4,
        view_translation: Vec3,
        view_backward: Vec3,
        far: float,
    ) -> Frustum:
        """Creates a frustum from clip-from-world matrix with a custom far plane.

        Args:
            clip_from_world: The clip-from-world (view-projection) matrix
            view_translation: Camera position in world space
            view_backward: Camera backward direction (opposite of view direction)
            far: Custom far plane distance

        Returns:
            A new Frustum with the specified far plane
        """

    def intersects_sphere(self, sphere: Sphere, intersect_far: bool) -> bool:
        """Checks if a sphere intersects with the frustum.

        Args:
            sphere: The bounding sphere to test.
            intersect_far: Whether to also check intersection with the far plane.

        Returns:
            True if the sphere intersects with the frustum.
        """

    def intersects_obb(
        self,
        aabb: Aabb,
        world_from_local: Affine3A,
        intersect_near: bool,
        intersect_far: bool,
    ) -> bool:
        """Checks if an oriented bounding box intersects with the frustum.

        Args:
            aabb: The axis-aligned bounding box.
            world_from_local: Transform from local to world space.
            intersect_near: Whether to also check intersection with the near plane.
            intersect_far: Whether to also check intersection with the far plane.

        Returns:
            True if the OBB intersects with the frustum.
        """

    def contains_aabb(self, aabb: Aabb, world_from_local: Affine3A) -> bool:
        """Checks if an oriented bounding box is completely contained within the frustum.

        Args:
            aabb: The axis-aligned bounding box.
            world_from_local: Transform from local to world space.

        Returns:
            True if the AABB is completely contained within the frustum.
        """

class SubCameraView:
    """Defines a sub-region of a camera's view for split-screen or tiled rendering.

    Attributes:
        full_size: Size of the full camera view in pixels
        offset: Offset of this sub-view from top-left of full view
        size: Size of this sub-view in pixels
    """

    def __init__(
        self,
        full_size: UVec2 | None = None,
        offset: Vec2 | None = None,
        size: UVec2 | None = None,
    ) -> None: ...

    @property
    def full_size(self) -> UVec2: ...
    @full_size.setter
    def full_size(self, value: UVec2) -> None: ...

    @property
    def offset(self) -> Vec2: ...
    @offset.setter
    def offset(self, value: Vec2) -> None: ...

    @property
    def size(self) -> UVec2: ...
    @size.setter
    def size(self, value: UVec2) -> None: ...

class PhysicalCameraParameters:
    """Physical camera parameters for exposure calculation.

    These parameters model a real camera's exposure settings:
    - Aperture (f-stops): Controls depth of field and light intake
    - Shutter speed: Exposure time in seconds
    - ISO sensitivity: Sensor sensitivity to light
    - Sensor height: Physical sensor height in meters
    """

    def __init__(
        self,
        aperture_f_stops: float = 1.0,
        shutter_speed_s: float = 1.0 / 125.0,
        sensitivity_iso: float = 100.0,
        sensor_height: float = 0.01866,
    ) -> None: ...

    @property
    def aperture_f_stops(self) -> float: ...
    @aperture_f_stops.setter
    def aperture_f_stops(self, value: float) -> None: ...

    @property
    def shutter_speed_s(self) -> float: ...
    @shutter_speed_s.setter
    def shutter_speed_s(self, value: float) -> None: ...

    @property
    def sensitivity_iso(self) -> float: ...
    @sensitivity_iso.setter
    def sensitivity_iso(self, value: float) -> None: ...

    @property
    def sensor_height(self) -> float: ...
    @sensor_height.setter
    def sensor_height(self, value: float) -> None: ...

    def ev100(self) -> float:
        """Calculate EV100 from physical camera parameters."""

class RenderLayers(Component):
    """Layer-based visibility filtering for cameras and entities.

    Cameras only render entities on matching layers.
    Uses bitset (32 layers max, 0-31). By default, entities and cameras
    are on layer 0.

    Examples:
        >>> # Create entity on layer 1 (hidden from default camera)
        >>> commands.spawn((
        ...     Mesh3d(cube_handle),
        ...     RenderLayers.layer(1)
        ... ))
        >>>
        >>> # Create camera that only sees layer 1
        >>> commands.spawn((
        ...     Camera3d(),
        ...     Camera(),
        ...     Transform.from_translation(Vec3(0.0, 2.0, 5.0)),
        ...     RenderLayers.layer(1)
        ... ))
        >>>
        >>> # Multi-layer setup (see layers 0 and 1)
        >>> layers = RenderLayers.layer(0).with_(1)
    """

    @staticmethod
    def all() -> RenderLayers:
        """Create RenderLayers with all layers enabled (0-31)."""

    @staticmethod
    def none() -> RenderLayers:
        """Create RenderLayers with no layers enabled."""

    @staticmethod
    def layer(layer: int) -> RenderLayers:
        """Create RenderLayers with only the specified layer enabled (0-31).

        Args:
            layer: Layer number (0-31)

        Returns:
            RenderLayers with only the specified layer enabled

        Raises:
            ValueError: If layer is not between 0 and 31
        """

    def with_(self, layer: int) -> RenderLayers:
        """Create a new RenderLayers with the specified layer added.

        Args:
            layer: Layer number to add (0-31)

        Returns:
            New RenderLayers with the layer added

        Raises:
            ValueError: If layer is not between 0 and 31
        """

    def without(self, layer: int) -> RenderLayers:
        """Create a new RenderLayers with the specified layer removed.

        Args:
            layer: Layer number to remove (0-31)

        Returns:
            New RenderLayers with the layer removed

        Raises:
            ValueError: If layer is not between 0 and 31
        """

    def intersects(self, other: RenderLayers) -> bool:
        """Check if this RenderLayers intersects with another.

        Two RenderLayers intersect if they have any common layers enabled.
        This is used by Bevy to determine if a camera should render an entity.

        Args:
            other: RenderLayers to check intersection with

        Returns:
            True if the layers have at least one common layer
        """

    def bits(self) -> list[int]:
        """Get the bitmask representation of enabled layers.

        Returns:
            List of u64 values representing the layer bitmask
        """

    def iter(self) -> list[int]:
        """Get all enabled layer indices.

        Returns:
            List of layer indices that are enabled
        """

    @property
    def active_layers(self) -> list[int]:
        """Get all enabled layer indices (alias for iter()).

        Returns:
            List of layer indices that are enabled
        """

    def intersection(self, other: RenderLayers) -> RenderLayers:
        """Get layers common to both RenderLayers (bitwise AND).

        Args:
            other: RenderLayers to intersect with

        Returns:
            New RenderLayers with only layers present in both
        """

    def union(self, other: RenderLayers) -> RenderLayers:
        """Get all layers in either RenderLayers (bitwise OR).

        Args:
            other: RenderLayers to union with

        Returns:
            New RenderLayers with all layers from both
        """

    def symmetric_difference(self, other: RenderLayers) -> RenderLayers:
        """Get layers in exactly one of the RenderLayers (bitwise XOR).

        Args:
            other: RenderLayers to compare with

        Returns:
            New RenderLayers with layers present in exactly one
        """

    @staticmethod
    def from_layers(layers: list[int]) -> RenderLayers:
        """Create RenderLayers from a list of layer indices.

        Args:
            layers: List of layer indices to enable

        Returns:
            RenderLayers with specified layers enabled
        """

    def __init__(self) -> None:
        """Create RenderLayers with layer 0 enabled (default)."""

    def __and__(self, other: RenderLayers) -> RenderLayers:
        """Bitwise AND of two RenderLayers (same as intersection)."""

    def __or__(self, other: RenderLayers) -> RenderLayers:
        """Bitwise OR of two RenderLayers (same as union)."""

    def __xor__(self, other: RenderLayers) -> RenderLayers:
        """Bitwise XOR of two RenderLayers (same as symmetric_difference)."""

class Skybox(Component):
    """Skybox component that displays an environment map as the background."""

    image: Handle[Image]
    brightness: float
    rotation: Quat

    def __init__(
        self,
        image: Handle[Image],
        brightness: float = 0.0,
        rotation: Quat = ...,
    ) -> None: ...

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        brightness: np.ndarray | None = None,
    ) -> Batchable: ...

class Exposure(Component):
    """Controls camera exposure for HDR rendering using EV100 values.

    Exposure determines how much light energy the camera absorbs. Higher EV100
    values result in darker images, lower values in brighter images. This is
    essential for HDR workflows and photorealistic rendering.

    EV100 (Exposure Value at ISO 100) is a standard photographic measure of
    scene luminance based on aperture, shutter speed, and ISO sensitivity.

    Examples:
        >>> # Use preset exposure values
        >>> exposure = Exposure.SUNLIGHT()  # Bright outdoor (EV100=15.0)
        >>> exposure = Exposure.INDOOR()    # Indoor lighting (EV100=7.0)
        >>> exposure = Exposure.BLENDER()   # Default Blender (EV100=9.7)
        >>>
        >>> # Create custom exposure
        >>> exposure = Exposure(ev100=12.0)
        >>>
        >>> # Adjust exposure in a system
        >>> def adjust_exposure(query: Query[Mut[Exposure]]) -> None:
        ...     for exposure in query:
        ...         exposure.ev100 += 0.1  # Slightly darken

    References:
        - https://en.wikipedia.org/wiki/Exposure_(photography)
        - https://en.wikipedia.org/wiki/Exposure_value#Tabulated_exposure_values
    """

    SUNLIGHT: ClassVar[Exposure]
    """Bright outdoor daylight exposure (EV100 = 15.0)"""

    OVERCAST: ClassVar[Exposure]
    """Overcast/cloudy conditions exposure (EV100 = 12.0)"""

    INDOOR: ClassVar[Exposure]
    """Indoor lighting exposure (EV100 = 7.0)"""

    BLENDER: ClassVar[Exposure]
    """Default Blender exposure (EV100 = 9.7) - reasonable default for most scenes"""

    EV100_SUNLIGHT: ClassVar[float]
    """EV100 constant for bright outdoor daylight (15.0)"""

    EV100_OVERCAST: ClassVar[float]
    """EV100 constant for overcast/cloudy conditions (12.0)"""

    EV100_INDOOR: ClassVar[float]
    """EV100 constant for indoor lighting (7.0)"""

    EV100_BLENDER: ClassVar[float]
    """EV100 constant matching Blender's default (9.7)"""

    def __init__(self, ev100: float = 9.7) -> None:
        """Create a new Exposure component.

        Args:
            ev100: Exposure Value at ISO 100. Higher values = darker images,
                   lower values = brighter images. Default is 9.7 (Blender preset).

        Examples:
            >>> # Default (Blender preset)
            >>> exposure = Exposure()
            >>>
            >>> # Custom exposure
            >>> exposure = Exposure(ev100=12.0)
        """

    @property
    def ev100(self) -> float:
        """Get the EV100 value.

        EV100 (Exposure Value at ISO 100) is a standard measure of scene luminance.
        Higher values result in darker images, lower values in brighter images.

        Returns:
            Current EV100 value
        """

    @ev100.setter
    def ev100(self, value: float) -> None:
        """Set the EV100 value.

        Args:
            value: New EV100 value
        """

    def exposure(self) -> float:
        """Get the exposure multiplier applied to HDR colors.

        This converts the EV100 value to an exposure multiplier that is
        actually applied to HDR colors during rendering. Formula: 2^(-ev100) / 1.2

        Returns:
            Exposure multiplier value used by the rendering pipeline

        References:
            https://google.github.io/filament/Filament.md.html#imagingpipeline/physicallybasedcamera/exposure
        """

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        ev100: np.ndarray | None = None,
    ) -> Batchable: ...

    @staticmethod
    def from_physical_camera(physical_camera_parameters: PhysicalCameraParameters) -> Exposure:
        """Create Exposure from physical camera parameters.

        Args:
            physical_camera_parameters: Physical camera settings (aperture, shutter speed, ISO, sensor size)

        Returns:
            Exposure component with EV100 calculated from physical parameters
        """

class PerspectiveProjection:
    """Perspective projection with field of view."""

    fov: float
    aspect_ratio: float
    near: float
    far: float
    near_clip_plane: Vec4
    """Custom near clipping plane (normal + distance), for mirror/reflection effects."""

    def __init__(
        self,
        fov: float | None = None,
        aspect_ratio: float | None = None,
        near: float | None = None,
        far: float | None = None,
    ) -> None: ...

    def get_clip_from_view(self) -> Mat4:
        """Generate the projection matrix.

        Returns:
            The clip-from-view (projection) matrix
        """

    def update(self, width: float, height: float) -> None:
        """Update projection for a new render area size.

        Updates the aspect ratio based on the new dimensions.

        Args:
            width: New width of the render area
            height: New height of the render area
        """

    def get_frustum_corners(self, z_near: float, z_far: float) -> list[Vec3A]:
        """Get the eight corners of the camera frustum.

        Args:
            z_near: Near plane distance
            z_far: Far plane distance

        Returns:
            List of 8 Vec3A corners in the order: bottom right, top right,
            top left, bottom left for near plane, then same for far plane.
        """

    def compute_frustum(self, camera_transform: GlobalTransform) -> Frustum:
        """Compute camera frustum from this projection and transform.

        Args:
            camera_transform: The camera's global transform

        Returns:
            The computed frustum for visibility culling
        """

    def get_clip_from_view_for_sub(self, sub_view: SubCameraView) -> Mat4:
        """Generate the projection matrix for a SubCameraView.

        Used when rendering a sub-region of the full camera view.

        Args:
            sub_view: The sub-camera view configuration

        Returns:
            The clip-from-view (projection) matrix adjusted for the sub-view
        """

class Projection(Component):
    """Camera projection (perspective or orthographic)."""

    def __init__(self) -> None: ...

    @staticmethod
    def Perspective(projection: PerspectiveProjection) -> Projection:
        """Create a perspective projection."""

    @staticmethod
    def Orthographic(projection: OrthographicProjection) -> Projection:
        """Create an orthographic projection."""

    def is_perspective(self) -> bool:
        """Check if this is a perspective projection."""

    def as_orthographic(self) -> OrthographicProjection:
        """Get the orthographic projection if this is orthographic.

        Raises:
            RuntimeError: If this is a perspective or custom projection.
        """

    def as_perspective(self) -> PerspectiveProjection:
        """Get the perspective projection if this is perspective.

        Raises:
            RuntimeError: If this is an orthographic or custom projection.
        """

    @property
    def orthographic_scale(self) -> float | None:
        """Get the orthographic scale if this is an orthographic projection.

        Returns None if this is not an orthographic projection.
        """

    @orthographic_scale.setter
    def orthographic_scale(self, scale: float) -> None:
        """Set the orthographic scale.

        Raises:
            RuntimeError: If this is not an orthographic projection.
        """

    @property
    def perspective_fov(self) -> float | None:
        """Get the perspective FOV if this is a perspective projection.

        Returns None if this is not a perspective projection.
        """

    @perspective_fov.setter
    def perspective_fov(self, fov: float) -> None:
        """Set the perspective FOV.

        Raises:
            RuntimeError: If this is not a perspective projection.
        """

class ClearColor(Resource):
    color: Color

    def __init__(self, color: Color = Color.WHITE) -> None: ...


class ClearColorConfig:
    """Configuration for clearing the screen background.

    Controls what color is used to clear the camera's render target.

    Example:
        ```python
        from pybevy.camera import Camera, ClearColorConfig
        from pybevy.color import Color

        # Use the default clear color (from ClearColor resource)
        camera = Camera()
        camera.clear_color = ClearColorConfig.Default()

        # Use a custom clear color
        camera.clear_color = ClearColorConfig.Custom(Color.srgb(0.2, 0.2, 0.2))

        # Don't clear at all (useful for layered rendering)
        camera.clear_color = ClearColorConfig.None_()
        ```
    """

    @staticmethod
    def Default() -> ClearColorConfig:
        """Use the default clear color from the ClearColor resource."""

    @staticmethod
    def Custom(color: Color) -> ClearColorConfig:
        """Use a custom clear color.

        Args:
            color: The color to clear the screen with
        """

    @staticmethod
    def None_() -> ClearColorConfig:
        """Don't clear the screen (useful for layered rendering)."""


class InheritedVisibility(Component):
    """Indicates if an entity is visible considering its ancestors.

    This is computed by propagating visibility down the entity hierarchy.
    If any ancestor is hidden, this will be false.

    This is a read-only component that is automatically computed by Bevy.
    You cannot spawn entities with custom InheritedVisibility values.

    Example:
        ```python
        from pybevy.camera import InheritedVisibility
        from pybevy.ecs import Query

        def check_visibility(query: Query[InheritedVisibility]) -> None:
            for inherited_vis in query:
                if inherited_vis.get():
                    print("Entity is visible (including all ancestors)")
        ```
    """
    VISIBLE: InheritedVisibility
    """Constant for visible state."""

    HIDDEN: InheritedVisibility
    """Constant for hidden state."""

    def get(self) -> bool:
        """Get whether the entity is visible considering its ancestors.

        Returns:
            True if the entity and all its ancestors are visible
        """

    def __bool__(self) -> bool:
        """Boolean conversion returns the visibility state."""

    def __eq__(self, other: object) -> bool:
        """Check equality with another InheritedVisibility."""


class ViewVisibility(Component):
    """Indicates if an entity should actually be rendered this frame.

    This is computed by Bevy's visibility system, taking into account:
    - The entity's Visibility component
    - InheritedVisibility from ancestors
    - Frustum culling
    - RenderLayers filtering

    This is a read-only component that is automatically computed by Bevy.
    You cannot spawn entities with custom ViewVisibility values.

    Example:
        ```python
        from pybevy.camera import ViewVisibility
        from pybevy.ecs import Query

        def check_rendered(query: Query[ViewVisibility]) -> None:
            for view_vis in query:
                if view_vis.get():
                    print("Entity will be rendered this frame")
        ```
    """
    HIDDEN: ViewVisibility
    """Constant for hidden state."""

    def get(self) -> bool:
        """Get whether the entity should be rendered this frame.

        Returns:
            True if the entity will be rendered this frame
        """

    def __bool__(self) -> bool:
        """Boolean conversion returns the visibility state."""

    def __eq__(self, other: object) -> bool:
        """Check equality with another ViewVisibility."""

class CameraMainTextureUsages(Component):
    """Controls the TextureUsages of the main texture generated for the camera.

    This allows you to configure how the camera's output texture can be used.
    By default, the texture can be used as a render attachment, sampled in shaders,
    and copied from.
    """

    COPY_SRC: int
    """Texture usage: can copy from this texture."""

    COPY_DST: int
    """Texture usage: can copy to this texture."""

    TEXTURE_BINDING: int
    """Texture usage: can be sampled in shaders."""

    STORAGE_BINDING: int
    """Texture usage: can be used as storage texture."""

    RENDER_ATTACHMENT: int
    """Texture usage: can be used as render target."""

    def __init__(self, flags: int = ...) -> None:
        """Create a new CameraMainTextureUsages with the specified texture usage flags.

        Args:
            flags: Texture usage flags as a bitmask.
        """

    @property
    def flags(self) -> int:
        """Get the texture usage flags as a bitmask."""

    def with_(self, usages: int) -> CameraMainTextureUsages:
        """Add additional texture usages to this component.

        Returns a new CameraMainTextureUsages with the combined flags.

        Args:
            usages: Texture usage flags to add as a bitmask.

        Returns:
            New CameraMainTextureUsages with combined usage flags.
        """

class MainPassResolutionOverride(Component):
    """Overrides the resolution of the main camera pass.

    When set on a camera entity, this will force the camera to render
    at the specified resolution, regardless of the window or viewport size.
    """

    def __init__(self, resolution: UVec2) -> None:
        """Create a new MainPassResolutionOverride with the specified resolution.

        Args:
            resolution: The resolution to use for the main pass as UVec2(width, height).
        """

    @property
    def resolution(self) -> UVec2:
        """Get the resolution as UVec2."""

    @property
    def width(self) -> int:
        """Get the width component of the resolution."""

    @property
    def height(self) -> int:
        """Get the height component of the resolution."""

class CubemapLayout:
    """Defines the order of images in a packed cubemap image.

    Different tools export cubemaps in different layouts. This enum helps
    specify how the 6 faces of a cubemap are arranged in a single image.
    """

    CrossVertical: CubemapLayout
    """Layout in a vertical cross format."""

    CrossHorizontal: CubemapLayout
    """Layout in a horizontal cross format."""

    SequenceVertical: CubemapLayout
    """Layout in a vertical sequence."""

    SequenceHorizontal: CubemapLayout
    """Layout in a horizontal sequence."""

class CubemapFrusta(Component):
    """Contains 6 frustums for cubemap rendering.

    Each frustum corresponds to one face of the cubemap:
    - Index 0: +X face
    - Index 1: -X face
    - Index 2: +Y face
    - Index 3: -Y face
    - Index 4: +Z face
    - Index 5: -Z face
    """

    def __init__(self) -> None: ...

    def frusta(self) -> list[Frustum]:
        """Get all 6 frustums as a list."""

    def get(self, index: int) -> Frustum:
        """Get the frustum at the specified index (0-5)."""

    def __len__(self) -> int:
        """Get the number of frustums (always 6)."""

class VisibleMeshEntities(Component):
    """Contains the list of mesh entities visible to a camera.

    This is automatically computed by Bevy's visibility system and contains
    all mesh entities that passed visibility checks for this camera.
    """

    def __init__(self) -> None: ...

    def entities(self) -> list[Entity]:
        """Get all visible entities as a list."""

    def __len__(self) -> int:
        """Get the number of visible entities."""

    def is_empty(self) -> bool:
        """Check if there are no visible entities."""

class CubemapVisibleEntities(Component):
    """Contains visible entities for each face of a cubemap.

    Contains 6 VisibleMeshEntities, one for each cubemap face:
    - Index 0: +X face
    - Index 1: -X face
    - Index 2: +Y face
    - Index 3: -Y face
    - Index 4: +Z face
    - Index 5: -Z face
    """

    def __init__(self) -> None: ...

    def get(self, i: int) -> VisibleMeshEntities:
        """Get the VisibleMeshEntities for a specific cubemap face (0-5)."""

    def __len__(self) -> int:
        """Get the number of faces (always 6)."""


class VisibilityClass(Component):
    """A bucket into which entities are grouped for visibility purposes.

    Bevy's rendering subsystems (3D, 2D, etc.) use visibility classes to
    quickly identify which entities they are responsible for rendering.
    This component stores which visibility class(es) an entity belongs to.

    Visibility classes are typically added automatically by hooks when
    adding renderable components like Mesh3d, Mesh2d, or Sprite.
    """

    def __init__(self) -> None: ...

    def __len__(self) -> int:
        """Get the number of visibility classes this entity belongs to."""

    def is_empty(self) -> bool:
        """Check if the entity has no visibility classes."""

    def contains(self, component_type: type[Component]) -> bool:
        """Check if the entity belongs to a specific component type's visibility class.

        Args:
            component_type: A component class (like Mesh3d, Mesh2d, Sprite).

        Returns:
            True if this entity belongs to that visibility class.
        """

    def add(self, component_type: type[Component]) -> None:
        """Add a visibility class for a component type.

        Args:
            component_type: A component class (like Mesh3d, Mesh2d, Sprite).
        """

    def clear(self) -> None:
        """Clear all visibility classes."""


class NormalizedRenderTarget:
    """A normalized render target used for equality comparisons.

    This is a normalized version of RenderTarget, mostly used for
    comparing whether two cameras render to the same target.

    Note: Window render targets are obtained from Camera methods, not
    constructed directly. You can construct Image, TextureView, and None
    render targets using the static methods.
    """

    @staticmethod
    def image(handle: Handle[Image], scale_factor: float = 1.0) -> NormalizedRenderTarget:
        """Create an image render target from an image handle."""

    @staticmethod
    def texture_view(texture_view_id: int) -> NormalizedRenderTarget:
        """Create a texture view render target from a manual texture view handle ID."""

    @staticmethod
    def none(width: int, height: int) -> NormalizedRenderTarget:
        """Create a 'none' render target that only renders prepasses."""

    def is_window(self) -> bool:
        """Check if this is a window render target."""

    def is_image(self) -> bool:
        """Check if this is an image render target."""

    def is_texture_view(self) -> bool:
        """Check if this is a texture view render target."""

    def is_none(self) -> bool:
        """Check if this is a 'none' render target."""

    def window_entity(self) -> Entity | None:
        """Get the window entity if this is a window render target."""

    def none_dimensions(self) -> tuple[int, int] | None:
        """Get the dimensions if this is a 'none' render target."""

    def texture_view_id(self) -> int | None:
        """Get the texture view handle ID if this is a texture view render target."""

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...


class DepthPrepass(Component):
    """If added to Camera3d, depth values are copied to a separate texture."""
    def __init__(self) -> None: ...

class NormalPrepass(Component):
    """If added to Camera3d, vertex world normals are copied to a separate texture."""
    def __init__(self) -> None: ...

class MotionVectorPrepass(Component):
    """If added to Camera3d, screen space motion vectors are copied to a separate texture."""
    def __init__(self) -> None: ...

class DeferredPrepass(Component):
    """If added to Camera3d, deferred materials are rendered to the deferred gbuffer texture."""
    def __init__(self) -> None: ...
