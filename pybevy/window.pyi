from enum import Enum
from typing import ClassVar, Final, Literal

import numpy as np

from pybevy.app import App, Plugin
from pybevy.ecs import Batchable, Component, Entity, Message, SystemSet
from pybevy.input import (
    ButtonState,
    KeyboardInput,
    MouseButton,
    MouseScrollUnit,
    TouchPhase,
)
from pybevy.math import CompassOctant, IVec2, UVec2, Vec2

ExitSystems: Final[SystemSet]

class ExitCondition(Enum):
    """Determines when the application should exit based on window state.

    Variants:
        OnPrimaryClosed: Exit when the primary window is closed
        OnAllClosed: Exit when all windows are closed (default)
        DontExit: Never exit automatically (manual control via AppExit event)
    """
    OnPrimaryClosed = ...
    OnAllClosed = ...
    DontExit = ...

class WindowRef:
    """A window render-target reference."""

    class Primary(WindowRef):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Entity(WindowRef):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: Entity
        def __init__(self, value: Entity) -> None: ...

    def normalize(self, primary_window: Entity | None = None) -> NormalizedWindowRef | None: ...

class NormalizedWindowRef:
    @property
    def entity(self) -> Entity: ...
    def __eq__(self, other: object) -> bool: ...

class WindowPlugin(Plugin):
    """Plugin for window management.

    Args:
        primary_window: Configuration for the primary window.
                       Use None to create no window (headless mode).
        exit_condition: When to exit the application. Use ExitCondition.DontExit
                       for headless mode to prevent automatic exit.

    Example (normal window):
        >>> from pybevy.window import WindowPlugin, Window, WindowResolution
        >>> plugin = WindowPlugin(
        ...     primary_window=Window(
        ...         title="My Game",
        ...         resolution=WindowResolution(1920, 1080)
        ...     )
        ... )

    Example (headless mode):
        >>> from pybevy.window import WindowPlugin, ExitCondition
        >>> plugin = WindowPlugin(
        ...     primary_window=None,
        ...     exit_condition=ExitCondition.DontExit
        ... )
    """
    def __init__(
        self,
        primary_window: Window | None = None,
        exit_condition: ExitCondition | None = None,
    ) -> None: ...
    def build(self, app: App) -> None: ...

# Plugin type markers for builder.disable() method
class AudioPlugin(Plugin):
    """Audio plugin - can be disabled for silent applications."""
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class WindowResolution:
    """
    Window resolution and DPI scaling.

    Manages logical and physical window dimensions and scale factor.
    Logical pixels are DPI-independent; physical pixels are actual screen pixels.
    """

    def __init__(
        self,
        physical_width: int = 1280,
        physical_height: int = 720,
        scale_factor_override: float | None = None,
    ) -> None:
        """Create a new window resolution from a size in physical pixels."""

    @property
    def width(self) -> float:
        """Get logical width in points."""

    @property
    def height(self) -> float:
        """Get logical height in points."""

    @property
    def physical_width(self) -> int:
        """Get physical width in pixels (width * scale_factor)."""

    @property
    def physical_height(self) -> int:
        """Get physical height in pixels (height * scale_factor)."""

    @property
    def scale_factor(self) -> float:
        """Get DPI scale factor (typically 1.0, 1.5, or 2.0 for high-DPI displays)."""

    def set(self, width: float, height: float) -> None:
        """Set logical width and height."""

    def set_physical_resolution(self, width: int, height: int) -> None:
        """Set physical pixel resolution."""

    def set_scale_factor_override(self, scale_factor_override: float | None) -> None:
        """Override the OS-provided scale factor."""

    def size(self) -> Vec2:
        """Get the logical size (width, height) as a Vec2."""

    def physical_size(self) -> UVec2:
        """Get the physical size (width, height) in pixels as a UVec2."""

    @property
    def base_scale_factor(self) -> float:
        """Get the base scale factor (set by window backend, e.g., HiDPI)."""

    def scale_factor_override(self) -> float | None:
        """Get the override scale factor, if any."""

    def set_scale_factor(self, scale_factor: float) -> None:
        """Set the base scale factor (used by window backend initialization)."""

    def with_scale_factor_override(self, scale_factor_override: float) -> WindowResolution:
        """Return a copy of this resolution with the given scale factor override."""

class VideoModeSelection:
    """Video mode selection for fullscreen windows.

    Controls which video mode (resolution, refresh rate) to use when
    the window is in fullscreen mode.

    Example:
        ```python
        from pybevy.window import WindowMode, MonitorSelection, VideoModeSelection

        # Fullscreen using current video mode (most common)
        mode = WindowMode.Fullscreen(
            monitor=MonitorSelection.Primary(),
            video_mode=VideoModeSelection.Current()
        )
        ```

    """

    class Current(VideoModeSelection):
        """Use the video mode that the monitor is already in."""
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Specific(VideoModeSelection):
        """Use a specific video mode from the monitor's supported modes."""
        __match_args__: ClassVar[tuple[Literal["video_mode"]]]
        video_mode: VideoMode
        def __init__(self, video_mode: VideoMode) -> None: ...


class WindowMode:
    """Window display mode.

    Example:
        ```python
        from pybevy.window import WindowMode, MonitorSelection, VideoModeSelection

        # Normal windowed mode
        mode = WindowMode.Windowed()

        # Borderless fullscreen on primary monitor
        mode = WindowMode.BorderlessFullscreen(MonitorSelection.Primary())

        # True fullscreen on second monitor
        mode = WindowMode.Fullscreen(
            monitor=MonitorSelection.Index(1),
            video_mode=VideoModeSelection.Current()
        )
        ```
    """

    class Windowed(WindowMode):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BorderlessFullscreen(WindowMode):
        __match_args__: ClassVar[tuple[Literal["monitor"]]]
        monitor: MonitorSelection
        def __init__(self, monitor: MonitorSelection) -> None: ...

    class Fullscreen(WindowMode):
        __match_args__: ClassVar[tuple[Literal["monitor"], Literal["video_mode"]]]
        monitor: MonitorSelection
        video_mode: VideoModeSelection
        def __init__(self, monitor: MonitorSelection, video_mode: VideoModeSelection) -> None: ...

class WindowLevel:
    """Window z-order level relative to other windows.

    Controls whether the window appears above or below other windows.

    Variants:
        AlwaysOnBottom: Window always below Normal and AlwaysOnTop windows (useful for widgets)
        Normal: Default window level
        AlwaysOnTop: Window always on top of Normal and AlwaysOnBottom windows
    """

    AlwaysOnBottom: WindowLevel
    Normal: WindowLevel
    AlwaysOnTop: WindowLevel

class Window(Component):
    """
    Window component for window property access and modification.

    Windows are entities in Bevy's ECS. The primary window is automatically
    created by DefaultPlugins and has the PrimaryWindow marker component.

    Example:
        ```python
        from pybevy.window import Window, PrimaryWindow, WindowMode, MonitorSelection, VideoModeSelection
        from pybevy.ecs import Single, Mut, With

        def toggle_fullscreen(
            window: Single[Mut[Window], With[PrimaryWindow]],
            keyboard: Res[ButtonInput],
        ) -> None:
            if keyboard.just_pressed(KeyCode.F11):
                if window.mode == WindowMode.Windowed():
                    window.mode = WindowMode.Fullscreen(
                        MonitorSelection.Primary(), VideoModeSelection.Current()
                    )
                else:
                    window.mode = WindowMode.Windowed()
        ```
    """

    def __init__(
        self,
        title: str = "PyBevy App",
        resolution: WindowResolution | None = None,
        decorations: bool = True,
        resizable: bool = True,
        mode: WindowMode | None = None,
        transparent: bool = False,
        window_level: WindowLevel | None = None,
    ) -> None:
        """Create a new window."""

    @property
    def title(self) -> str:
        """Get window title."""

    @title.setter
    def title(self, title: str) -> None:
        """Set window title."""

    @property
    def resolution(self) -> WindowResolution:
        """Get window resolution."""

    @resolution.setter
    def resolution(self, resolution: WindowResolution) -> None:
        """Set window resolution."""

    @property
    def focused(self) -> bool:
        """Get whether window has input focus (read-only)."""

    @property
    def decorations(self) -> bool:
        """Get whether window shows title bar and borders."""

    @decorations.setter
    def decorations(self, decorations: bool) -> None:
        """Set whether window shows title bar and borders."""

    @property
    def resizable(self) -> bool:
        """Get whether user can resize window."""

    @resizable.setter
    def resizable(self, resizable: bool) -> None:
        """Set whether user can resize window."""

    @property
    def mode(self) -> WindowMode:
        """Get window mode."""

    @mode.setter
    def mode(self, mode: WindowMode) -> None:
        """Set window mode."""

    @property
    def window_level(self) -> WindowLevel:
        """Get window z-order level."""

    @window_level.setter
    def window_level(self, window_level: WindowLevel) -> None:
        """Set window z-order level."""

    @property
    def transparent(self) -> bool:
        """Get whether window has transparent background."""

    @transparent.setter
    def transparent(self, value: bool) -> None:
        """Set whether window has transparent background."""

    @property
    def visible(self) -> bool:
        """Get whether window is visible."""

    @visible.setter
    def visible(self, value: bool) -> None:
        """Set whether window is visible."""

    @property
    def skip_taskbar(self) -> bool:
        """Get whether window is hidden from taskbar."""

    @skip_taskbar.setter
    def skip_taskbar(self, value: bool) -> None:
        """Set whether window is hidden from taskbar."""

    @property
    def ime_enabled(self) -> bool:
        """Get whether IME (Input Method Editor) is enabled."""

    @ime_enabled.setter
    def ime_enabled(self, value: bool) -> None:
        """Set whether IME (Input Method Editor) is enabled."""

    @property
    def has_shadow(self) -> bool:
        """Get whether window has drop shadow (macOS only)."""

    @has_shadow.setter
    def has_shadow(self, value: bool) -> None:
        """Set whether window has drop shadow (macOS only)."""

    @property
    def name(self) -> str | None:
        """Get window name (optional identifier)."""

    @name.setter
    def name(self, value: str | None) -> None:
        """Set window name (optional identifier)."""

    @property
    def clip_children(self) -> bool:
        """Whether to clip UI content that overflows the window bounds."""

    @clip_children.setter
    def clip_children(self, value: bool) -> None:
        """Set whether to clip UI content."""

    @property
    def fit_canvas_to_parent(self) -> bool:
        """Whether to fit the canvas to its parent element (web only)."""

    @fit_canvas_to_parent.setter
    def fit_canvas_to_parent(self, value: bool) -> None:
        """Set whether to fit canvas to parent."""

    @property
    def prevent_default_event_handling(self) -> bool:
        """Whether to prevent default event handling (web only)."""

    @prevent_default_event_handling.setter
    def prevent_default_event_handling(self, value: bool) -> None:
        """Set whether to prevent default event handling."""

    @property
    def ime_position(self) -> Vec2:
        """Get the position of the IME (Input Method Editor) window."""

    @ime_position.setter
    def ime_position(self, value: Vec2) -> None:
        """Set the IME window position."""

    @property
    def movable_by_window_background(self) -> bool:
        """Whether window can be moved by dragging its background (macOS only)."""

    @movable_by_window_background.setter
    def movable_by_window_background(self, value: bool) -> None:
        """Set whether window can be moved by background drag."""

    @property
    def fullsize_content_view(self) -> bool:
        """Whether to use fullsize content view (macOS only)."""

    @fullsize_content_view.setter
    def fullsize_content_view(self, value: bool) -> None:
        """Set fullsize content view mode."""

    @property
    def recognize_doubletap_gesture(self) -> bool:
        """Whether to recognize double-tap gestures (iOS only)."""

    @recognize_doubletap_gesture.setter
    def recognize_doubletap_gesture(self, value: bool) -> None:
        """Set whether to recognize double-tap gestures."""

    @property
    def recognize_pinch_gesture(self) -> bool:
        """Whether to recognize pinch gestures (iOS only)."""

    @recognize_pinch_gesture.setter
    def recognize_pinch_gesture(self, value: bool) -> None:
        """Set whether to recognize pinch gestures."""

    @property
    def recognize_pan_gesture(self) -> tuple[int, int] | None:
        """Pan gesture configuration (min_fingers, max_fingers) or None (iOS only)."""

    @recognize_pan_gesture.setter
    def recognize_pan_gesture(self, value: tuple[int, int] | None) -> None:
        """Set pan gesture configuration."""

    @property
    def prefers_home_indicator_hidden(self) -> bool:
        """Whether to hide the home indicator (iOS only)."""

    @prefers_home_indicator_hidden.setter
    def prefers_home_indicator_hidden(self, value: bool) -> None:
        """Set home indicator visibility preference."""

    @property
    def prefers_status_bar_hidden(self) -> bool:
        """Whether to hide the status bar (iOS only)."""

    @prefers_status_bar_hidden.setter
    def prefers_status_bar_hidden(self, value: bool) -> None:
        """Set status bar visibility preference."""

    @property
    def canvas(self) -> str | None:
        """Get the canvas element selector (web only)."""

    @canvas.setter
    def canvas(self, value: str | None) -> None:
        """Set the canvas element selector."""

    @property
    def recognize_rotation_gesture(self) -> bool:
        """Whether to recognize rotation gestures (iOS only)."""

    @recognize_rotation_gesture.setter
    def recognize_rotation_gesture(self, value: bool) -> None:
        """Set whether to recognize rotation gestures."""

    @property
    def titlebar_shown(self) -> bool:
        """Whether the titlebar is shown (macOS only)."""

    @titlebar_shown.setter
    def titlebar_shown(self, value: bool) -> None:
        """Set whether the titlebar is shown."""

    @property
    def titlebar_transparent(self) -> bool:
        """Whether the titlebar is transparent (macOS only)."""

    @titlebar_transparent.setter
    def titlebar_transparent(self, value: bool) -> None:
        """Set whether the titlebar is transparent."""

    @property
    def titlebar_show_title(self) -> bool:
        """Whether to show the title in the titlebar (macOS only)."""

    @titlebar_show_title.setter
    def titlebar_show_title(self, value: bool) -> None:
        """Set whether to show the title in the titlebar."""

    @property
    def titlebar_show_buttons(self) -> bool:
        """Whether to show window buttons in the titlebar (macOS only)."""

    @titlebar_show_buttons.setter
    def titlebar_show_buttons(self, value: bool) -> None:
        """Set whether to show window buttons in the titlebar."""

    @property
    def window_theme(self) -> WindowTheme | None:
        """Get the window theme (light/dark)."""

    @window_theme.setter
    def window_theme(self, value: WindowTheme | None) -> None:
        """Set the window theme."""

    @property
    def enabled_buttons(self) -> EnabledButtons:
        """Get which window buttons are enabled (minimize, maximize, close)."""

    @enabled_buttons.setter
    def enabled_buttons(self, value: EnabledButtons) -> None:
        """Set which window buttons are enabled."""

    @property
    def position(self) -> WindowPosition:
        """Get the window position."""

    @position.setter
    def position(self, value: WindowPosition) -> None:
        """Set the window position."""

    @property
    def present_mode(self) -> PresentMode:
        """Get the presentation/VSync mode."""

    @present_mode.setter
    def present_mode(self, value: PresentMode) -> None:
        """Set the presentation/VSync mode."""

    @property
    def composite_alpha_mode(self) -> CompositeAlphaMode:
        """Get the alpha compositing mode."""

    @composite_alpha_mode.setter
    def composite_alpha_mode(self, value: CompositeAlphaMode) -> None:
        """Set the alpha compositing mode."""

    @property
    def borderless_game(self) -> bool:
        """Whether this is a borderless game window (macOS)."""

    @borderless_game.setter
    def borderless_game(self, value: bool) -> None:
        """Set whether this is a borderless game window (macOS)."""

    @property
    def desired_maximum_frame_latency(self) -> int | None:
        """Desired maximum frame latency in frames (``None`` lets the GPU choose)."""

    @desired_maximum_frame_latency.setter
    def desired_maximum_frame_latency(self, value: int | None) -> None:
        """Set the desired maximum frame latency; must be ``None`` or at least 1."""

    @property
    def resize_constraints(self) -> WindowResizeConstraints:
        """Get the window resize constraints."""

    @resize_constraints.setter
    def resize_constraints(self, value: WindowResizeConstraints) -> None:
        """Set the window resize constraints."""

    @property
    def preferred_screen_edges_deferring_system_gestures(self) -> ScreenEdge:
        """Get screen edges that defer system gestures (iOS only)."""

    @preferred_screen_edges_deferring_system_gestures.setter
    def preferred_screen_edges_deferring_system_gestures(self, value: ScreenEdge) -> None:
        """Set screen edges that defer system gestures (iOS only)."""

    # Dimension methods

    def width(self) -> float:
        """Get the logical width of the window in points."""

    def height(self) -> float:
        """Get the logical height of the window in points."""

    def size(self) -> Vec2:
        """Get the logical size of the window as (width, height)."""

    def physical_width(self) -> int:
        """Get the physical width of the window in pixels."""

    def physical_height(self) -> int:
        """Get the physical height of the window in pixels."""

    def physical_size(self) -> UVec2:
        """Get the physical size of the window as (width, height)."""

    def scale_factor(self) -> float:
        """Get the scale factor (DPI) of the window."""

    def cursor_position(self) -> Vec2 | None:
        """Get cursor position in logical pixels, or None if outside window."""

    def physical_cursor_position(self) -> Vec2 | None:
        """Get cursor position in physical pixels, or None if outside window."""

    def set_cursor_position(self, position: Vec2 | None) -> None:
        """Set cursor position in logical pixels."""

    def set_maximized(self, maximized: bool) -> None:
        """Set whether the window is maximized."""

    def set_minimized(self, minimized: bool) -> None:
        """Set whether the window is minimized."""

    def start_drag_move(self) -> None:
        """Initiate a drag-move of the window.

        Must be called after a left mouse button press.
        There is no guarantee this will work unless the left mouse button
        was pressed immediately before this function was called.
        """

    def start_drag_resize(self, direction: CompassOctant) -> None:
        """Initiate a drag-resize of the window in the specified direction.

        Must be called after a left mouse button press.
        There is no guarantee this will work unless the left mouse button
        was pressed immediately before this function was called.

        Args:
            direction: The compass direction to resize toward (e.g., CompassOctant.SouthEast)
        """

    def set_physical_cursor_position(self, position: tuple[float, float] | None) -> None:
        """Set the cursor position in physical pixels.

        Args:
            position: (x, y) coordinates in physical pixels, or None to release
        """

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        decorations: np.typing.ArrayLike | None = None,
        resizable: np.typing.ArrayLike | None = None,
        transparent: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class PrimaryWindow(Component):
    """
    Marker component identifying the primary window.

    This component is automatically added to the window entity created by DefaultPlugins.
    Use With[PrimaryWindow] filter to query the main window.

    Example:
        ```python
        from pybevy.window import Window, PrimaryWindow
        from pybevy.ecs import Single, With

        def check_window(window: Single[Window, With[PrimaryWindow]]) -> None:
            print(f"Window: {window.title}")
        ```
    """

    def __init__(self) -> None:
        """Create a PrimaryWindow marker."""

class PrimaryMonitor(Component):
    """
    Marker component identifying the primary monitor.

    This component is automatically added by Bevy to the monitor entity
    representing the primary display.

    Example:
        ```python
        from pybevy.window import PrimaryMonitor
        from pybevy.ecs import Single, With

        def check_primary_monitor(entity: Single[Entity, With[PrimaryMonitor]]) -> None:
            print(f"Primary monitor entity: {entity}")
        ```
    """

    def __init__(self) -> None:
        """Create a PrimaryMonitor marker."""

class CursorGrabMode:
    """
    Cursor grab mode - controls whether cursor is locked or confined to window.

    Variants:
        None_: Cursor can move freely
        Confined: Cursor is confined to the window bounds but can move within it
        Locked: Cursor is locked to the center of the window (for FPS camera control)

    Note: bevy spells this variant `None`, which is a Python keyword, so it is
    exposed as `None_` in both the stubs and at runtime.
    """

    None_: CursorGrabMode
    Confined: CursorGrabMode
    Locked: CursorGrabMode

    def __hash__(self) -> int: ...

class CursorOptions(Component):
    """
    Cursor options component for window cursor control.

    Controls cursor visibility, grab mode, and hit testing.
    Query this component from Window entities to modify cursor behavior.

    Example:
        ```python
        from pybevy.window import CursorOptions, CursorGrabMode

        def grab_mouse(
            cursor_options: Single[Mut[CursorOptions]],
            mouse: Res[ButtonInput],
        ) -> None:
            if mouse.just_pressed(MouseButton.Left()):
                cursor_options.visible = False
                cursor_options.grab_mode = CursorGrabMode.Locked
        ```
    """

    def __init__(
        self,
        visible: bool = True,
        grab_mode: CursorGrabMode = CursorGrabMode.None_,
        hit_test: bool = True,
    ) -> None:
        """Create new cursor options with default settings."""

    @property
    def visible(self) -> bool:
        """Get cursor visibility."""

    @visible.setter
    def visible(self, visible: bool) -> None:
        """Set cursor visibility."""

    @property
    def grab_mode(self) -> CursorGrabMode:
        """Get cursor grab mode."""

    @grab_mode.setter
    def grab_mode(self, mode: CursorGrabMode) -> None:
        """Set cursor grab mode."""

    @property
    def hit_test(self) -> bool:
        """Get whether cursor events are captured by the window."""

    @hit_test.setter
    def hit_test(self, hit_test: bool) -> None:
        """Set whether cursor events are captured by the window."""

class CursorIcon(Component):
    """
    Cursor icon component for window cursor appearance.

    Insert into a window entity to set the cursor icon. Supports system
    cursors and (with custom_cursor feature) custom cursor images.

    Example:
        ```python
        from pybevy.window import CursorIcon, SystemCursorIcon, PrimaryWindow
        from pybevy.ecs import Single, Mut, With

        def set_pointer_cursor(
            cursor: Single[Mut[CursorIcon], With[PrimaryWindow]],
        ) -> None:
            cursor.set_system(SystemCursorIcon.Pointer)
        ```
    """

    def __init__(self, system_icon: SystemCursorIcon = ...) -> None:
        """Create a CursorIcon with a system cursor (default: Default)."""

    @staticmethod
    def system(icon: SystemCursorIcon) -> CursorIcon:
        """Create a CursorIcon from a system cursor icon."""

    @property
    def as_system(self) -> SystemCursorIcon | None:
        """Get the system cursor icon if this is a system cursor."""

    def set_system(self, icon: SystemCursorIcon) -> None:
        """Set this cursor to a system cursor icon."""

    def is_system(self) -> bool:
        """Check if this is a system cursor."""


class SystemCursorIcon:
    """
    System cursor icon types.

    Standard cursor appearances provided by the operating system.
    """

    Default: SystemCursorIcon
    ContextMenu: SystemCursorIcon
    Help: SystemCursorIcon
    Pointer: SystemCursorIcon
    Progress: SystemCursorIcon
    Wait: SystemCursorIcon
    Cell: SystemCursorIcon
    Crosshair: SystemCursorIcon
    Text: SystemCursorIcon
    VerticalText: SystemCursorIcon
    Alias: SystemCursorIcon
    Copy: SystemCursorIcon
    Move: SystemCursorIcon
    NoDrop: SystemCursorIcon
    NotAllowed: SystemCursorIcon
    Grab: SystemCursorIcon
    Grabbing: SystemCursorIcon
    EResize: SystemCursorIcon
    NResize: SystemCursorIcon
    NeResize: SystemCursorIcon
    NwResize: SystemCursorIcon
    SResize: SystemCursorIcon
    SeResize: SystemCursorIcon
    SwResize: SystemCursorIcon
    WResize: SystemCursorIcon
    EwResize: SystemCursorIcon
    NsResize: SystemCursorIcon
    NeswResize: SystemCursorIcon
    NwseResize: SystemCursorIcon
    ColResize: SystemCursorIcon
    RowResize: SystemCursorIcon
    AllScroll: SystemCursorIcon
    ZoomIn: SystemCursorIcon
    ZoomOut: SystemCursorIcon

    def __hash__(self) -> int: ...

class WindowTheme:
    """System window theme preference.

    Controls whether the window frame/decorations use light or dark theme.

    Example:
        ```python
        from pybevy.window import Window, WindowTheme

        window = Window()
        window.window_theme = WindowTheme.Dark  # Dark window decorations
        ```
    """
    Light: WindowTheme
    """Light theme (light window decorations)."""

    Dark: WindowTheme
    """Dark theme (dark window decorations)."""


class PresentMode:
    """Presentation mode for window surface synchronization.

    Controls VSync and screen tearing behavior.

    Example:
        ```python
        from pybevy.window import Window, PresentMode

        window = Window()
        window.present_mode = PresentMode.AutoVsync  # Enable VSync
        ```
    """
    AutoVsync: PresentMode
    """Automatic VSync - uses FifoRelaxed -> Fifo based on availability."""

    AutoNoVsync: PresentMode
    """Automatic no VSync - uses Immediate -> Mailbox -> Fifo based on availability."""

    Fifo: PresentMode
    """FIFO queue (traditional VSync). No tearing, may block. Default."""

    FifoRelaxed: PresentMode
    """Relaxed FIFO (adaptive VSync). May tear if frames take too long."""

    Immediate: PresentMode
    """No VSync. Tearing may occur. Lowest latency."""

    Mailbox: PresentMode
    """Single-frame queue (fast VSync). No tearing, newest frame used."""

    def __hash__(self) -> int: ...

class CompositeAlphaMode:
    """Alpha compositing mode for window surfaces.

    Controls how the window's alpha channel is handled during compositing.

    Example:
        ```python
        from pybevy.window import Window, CompositeAlphaMode

        window = Window()
        window.composite_alpha_mode = CompositeAlphaMode.Opaque
        ```
    """
    Auto: CompositeAlphaMode
    """Automatic - chooses Opaque or Inherit based on surface support. Default."""

    Opaque: CompositeAlphaMode
    """Alpha channel ignored, treated as 1.0."""

    PreMultiplied: CompositeAlphaMode
    """Alpha respected, color channels already multiplied by alpha."""

    PostMultiplied: CompositeAlphaMode
    """Alpha respected, compositor multiplies color channels by alpha."""

    Inherit: CompositeAlphaMode
    """Platform-specific default alpha handling."""

    def __hash__(self) -> int: ...

class WindowResizeConstraints:
    """Constraints on window resizing dimensions.

    Example:
        ```python
        from pybevy.window import Window, WindowResizeConstraints

        window = Window()
        window.resize_constraints = WindowResizeConstraints(
            min_width=800, min_height=600,
            max_width=1920, max_height=1080
        )
        ```
    """

    def __init__(
        self,
        min_width: float = 180.0,
        min_height: float = 120.0,
        max_width: float = ...,  # f32::INFINITY
        max_height: float = ...,  # f32::INFINITY
    ) -> None:
        """Create window resize constraints."""

    @property
    def min_width(self) -> float:
        """Minimum window width."""

    @min_width.setter
    def min_width(self, value: float) -> None:
        """Set minimum window width."""

    @property
    def min_height(self) -> float:
        """Minimum window height."""

    @min_height.setter
    def min_height(self, value: float) -> None:
        """Set minimum window height."""

    @property
    def max_width(self) -> float:
        """Maximum window width."""

    @max_width.setter
    def max_width(self, value: float) -> None:
        """Set maximum window width."""

    @property
    def max_height(self) -> float:
        """Maximum window height."""

    @max_height.setter
    def max_height(self, value: float) -> None:
        """Set maximum window height."""


class ScreenEdge:
    """Screen edge for iOS system gesture deferral.

    Only used on iOS. Specifies which screen edges should defer system gestures
    to allow app gestures to take priority.

    Variants:
        None: No edge (default)
        Top: Top edge of the screen
        Left: Left edge of the screen
        Bottom: Bottom edge of the screen
        Right: Right edge of the screen
        All: All edges of the screen
    """
    None_: ScreenEdge
    """No edge (default). Spelled `None_` because bevy's `None` is a Python keyword."""

    Top: ScreenEdge
    """Top edge of the screen."""

    Left: ScreenEdge
    """Left edge of the screen."""

    Bottom: ScreenEdge
    """Bottom edge of the screen."""

    Right: ScreenEdge
    """Right edge of the screen."""

    All: ScreenEdge
    """All edges of the screen."""

    def __hash__(self) -> int: ...

class AppLifecycle:
    """Application lifecycle state.

    Represents the current state of the application's lifecycle,
    useful for handling pause/resume events on mobile platforms.

    Example:
        ```python
        from pybevy.window import AppLifecycle
        from pybevy.ecs import Res

        def check_lifecycle(lifecycle: Res[AppLifecycle]) -> None:
            if lifecycle == AppLifecycle.Suspended:
                print("App is suspended (e.g., user switched away)")
            if lifecycle.is_active():
                print("App is active and can update")
        ```
    """
    Idle: AppLifecycle
    """Application is idle (initial state)."""

    Running: AppLifecycle
    """Application is running normally."""

    WillSuspend: AppLifecycle
    """Application is about to be suspended."""

    Suspended: AppLifecycle
    """Application is suspended (e.g., minimized or backgrounded)."""

    WillResume: AppLifecycle
    """Application is about to resume from suspended state."""

    def is_active(self) -> bool:
        """Returns True if the app can be updated.

        Active states: Running, WillSuspend, WillResume.
        Inactive states: Idle, Suspended.
        """


class MonitorSelection:
    """Selection criteria for choosing a monitor.

    Used when specifying which monitor a window should appear on.

    Example:
        ```python
        from pybevy.window import Window, MonitorSelection

        # Place window on the current monitor
        window = Window()
        window.monitor_selection = MonitorSelection.Current()

        # Place window on the primary monitor
        window.monitor_selection = MonitorSelection.Primary()

        # Place window on a specific monitor by index
        window.monitor_selection = MonitorSelection.Index(1)
        ```
    """
    class Current(MonitorSelection):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Primary(MonitorSelection):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Index(MonitorSelection):
        __match_args__: ClassVar[tuple[Literal["index"]]]
        index: int
        def __init__(self, index: int) -> None: ...

    class Entity(MonitorSelection):
        __match_args__: ClassVar[tuple[Literal["entity"]]]
        entity: Entity
        def __init__(self, entity: Entity) -> None: ...


class EnabledButtons:
    """Controls which window control buttons are enabled.

    Note: iOS, Android, and web platforms don't support window control buttons.
    Some Linux environments may ignore these settings.

    Example:
        ```python
        from pybevy.window import EnabledButtons

        # Disable maximize button
        buttons = EnabledButtons(minimize=True, maximize=False, close=True)
        ```
    """

    minimize: bool
    """Whether the minimize button is enabled."""

    maximize: bool
    """Whether the maximize button is enabled."""

    close: bool
    """Whether the close button is enabled."""

    def __init__(
        self,
        minimize: bool = True,
        maximize: bool = True,
        close: bool = True,
    ) -> None:
        """Create enabled buttons configuration.

        Args:
            minimize: Enable minimize button (default True)
            maximize: Enable maximize button (default True)
            close: Enable close button (default True)
        """


class WindowPosition:
    """Window position on screen.

    Example:
        ```python
        from pybevy.window import WindowPosition, MonitorSelection

        # Let the window manager decide
        pos = WindowPosition.Automatic()

        # Position at specific coordinates (physical pixels)
        pos = WindowPosition.At(IVec2(100, 200))

        # Center on primary monitor
        pos = WindowPosition.Centered(MonitorSelection.Primary())
        ```
    """

    class Automatic(WindowPosition):
        """Let the window manager select the position."""
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Centered(WindowPosition):
        """Center the window on the selected monitor."""
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: MonitorSelection
        def __init__(self, value: MonitorSelection) -> None: ...

    class At(WindowPosition):
        """Place the window at physical pixel coordinates."""
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: IVec2
        def __init__(self, value: IVec2) -> None: ...


class VideoMode:
    """A video mode supported by a monitor.

    Contains information about a display resolution, bit depth, and refresh rate
    that a monitor can support.

    Example:
        ```python
        from pybevy.window import Monitor, PrimaryMonitor
        from pybevy.ecs import Single, With

        def list_video_modes(monitor: Single[Monitor, With[PrimaryMonitor]]) -> None:
            for mode in monitor.video_modes:
                print(f"{mode.physical_size.x}x{mode.physical_size.y} @ {mode.refresh_rate_millihertz / 1000}Hz")
        ```
    """

    def __init__(
        self,
        physical_size: UVec2,
        bit_depth: int,
        refresh_rate_millihertz: int,
    ) -> None:
        """Create a new video mode.

        Args:
            physical_size: Resolution in physical pixels
            bit_depth: Color bit depth (typically 24 or 32)
            refresh_rate_millihertz: Refresh rate in millihertz (e.g., 60000 = 60Hz)
        """

    @property
    def physical_size(self) -> UVec2:
        """Get the resolution in physical pixels."""

    @property
    def bit_depth(self) -> int:
        """Get the color bit depth."""

    @property
    def refresh_rate_millihertz(self) -> int:
        """Get the refresh rate in millihertz."""


class Monitor(Component):
    """Monitor information component (read-only).

    Contains information about a display monitor. Monitor entities are created
    automatically by Bevy when the window system initializes.

    Note: This component is read-only. You cannot spawn Monitor entities from Python.

    Example:
        ```python
        from pybevy.window import Monitor, PrimaryMonitor
        from pybevy.ecs import Single, With

        def print_monitor_info(monitor: Single[Monitor, With[PrimaryMonitor]]) -> None:
            print(f"Primary monitor: {monitor.name}")
            print(f"Resolution: {monitor.physical_width}x{monitor.physical_height}")
            print(f"Scale factor: {monitor.scale_factor}")
            print(f"Refresh rate: {monitor.refresh_rate_millihertz / 1000 if monitor.refresh_rate_millihertz else 'unknown'}Hz")
        ```
    """

    @property
    def name(self) -> str | None:
        """Get the monitor name (if available)."""

    @property
    def physical_height(self) -> int:
        """Get the physical height in pixels."""

    @property
    def physical_width(self) -> int:
        """Get the physical width in pixels."""

    @property
    def physical_position(self) -> IVec2:
        """Get the physical position of the monitor on the virtual screen space."""

    @property
    def refresh_rate_millihertz(self) -> int | None:
        """Get the refresh rate in millihertz, if available."""

    @property
    def scale_factor(self) -> float:
        """Get the DPI scale factor."""

    @property
    def video_modes(self) -> list[VideoMode]:
        """Get the list of supported video modes."""

    def physical_size(self) -> UVec2:
        """Get the physical size as UVec2."""


class Ime(Message):
    """Input Method Editor event (read-only).

    This event is sent when IME is active on a window. IME is used for
    inputting text in languages like Chinese, Japanese, and Korean.

    Enable IME on a window by setting `window.ime_enabled = True`.

    Variants:
        Preedit: Composing text that should be shown at cursor position
        Commit: Finalized text that should be inserted
        Enabled: IME was enabled on the window
        Disabled: IME was disabled on the window

    Example:
        ```python
        from pybevy.ecs import MessageReader
        from pybevy.window import Ime

        def handle_ime(ime_events: MessageReader[Ime]) -> None:
            for ime in ime_events:
                match ime:
                    case Ime.Preedit(_, value, cursor):
                        print(f"Composing: {value}, cursor={cursor}")
                    case Ime.Commit(_, value):
                        print(f"Insert: {value}")
                    case Ime.Enabled():
                        print("IME enabled")
                    case Ime.Disabled():
                        print("IME disabled")
        ```
    """

    class Preedit(Ime):
        __match_args__: ClassVar[
            tuple[Literal["window"], Literal["value"], Literal["cursor"]]
        ]
        window: Entity
        value: str
        cursor: tuple[int, int] | None
        def __init__(
            self,
            window: Entity,
            value: str,
            cursor: tuple[int, int] | None = None,
        ) -> None: ...

    class Commit(Ime):
        __match_args__: ClassVar[tuple[Literal["window"], Literal["value"]]]
        window: Entity
        value: str
        def __init__(self, window: Entity, value: str) -> None: ...

    class Enabled(Ime):
        __match_args__: ClassVar[tuple[Literal["window"]]]
        window: Entity
        def __init__(self, window: Entity) -> None: ...

    class Disabled(Ime):
        __match_args__: ClassVar[tuple[Literal["window"]]]
        window: Entity
        def __init__(self, window: Entity) -> None: ...


class FileDragAndDrop(Message):
    """File drag and drop event (read-only).

    This event is sent when files are dragged over or dropped into a window.

    Variants:
        DroppedFile: A file was dropped into the window
        HoveredFile: A file is being hovered over the window
        HoveredFileCanceled: The file hover was canceled

    Example:
        ```python
        from pybevy.ecs import MessageReader
        from pybevy.window import FileDragAndDrop

        def handle_file_drop(events: MessageReader[FileDragAndDrop]) -> None:
            for event in events:
                match event:
                    case FileDragAndDrop.DroppedFile(_, path_buf):
                        print(f"File dropped: {path_buf}")
                    case FileDragAndDrop.HoveredFile(_, path_buf):
                        print(f"File hovering: {path_buf}")
                    case FileDragAndDrop.HoveredFileCanceled():
                        print("File hover canceled")
        ```
    """

    class DroppedFile(FileDragAndDrop):
        __match_args__: ClassVar[tuple[Literal["window"], Literal["path_buf"]]]
        window: Entity
        path_buf: str
        def __init__(self, window: Entity, path_buf: str) -> None: ...

    class HoveredFile(FileDragAndDrop):
        __match_args__: ClassVar[tuple[Literal["window"], Literal["path_buf"]]]
        window: Entity
        path_buf: str
        def __init__(self, window: Entity, path_buf: str) -> None: ...

    class HoveredFileCanceled(FileDragAndDrop):
        __match_args__: ClassVar[tuple[Literal["window"]]]
        window: Entity
        def __init__(self, window: Entity) -> None: ...

    def __eq__(self, other: object) -> bool: ...


class CursorEntered(Message):
    """Cursor entered window event (read-only).

    Sent when the user's cursor enters a window.

    Example:
        ```python
        from pybevy.ecs import MessageReader
        from pybevy.window import CursorEntered

        def handle_cursor_enter(events: MessageReader[CursorEntered]) -> None:
            for event in events:
                print(f"Cursor entered window: {event.window}")
        ```
    """

    def __init__(self, window: Entity) -> None:
        """Create a CursorEntered event."""

    @property
    def window(self) -> Entity:
        """Get the window entity that the cursor entered."""


class CursorLeft(Message):
    """Cursor left window event (read-only).

    Sent when the user's cursor leaves a window.

    Example:
        ```python
        from pybevy.ecs import MessageReader
        from pybevy.window import CursorLeft

        def handle_cursor_leave(events: MessageReader[CursorLeft]) -> None:
            for event in events:
                print(f"Cursor left window: {event.window}")
        ```
    """

    def __init__(self, window: Entity) -> None:
        """Create a CursorLeft event."""

    @property
    def window(self) -> Entity:
        """Get the window entity that the cursor left."""


class CursorMoved(Message):
    """
    Cursor position event message.

    Contains information about cursor movement with absolute position in the window.
    Use with MessageReader to receive cursor position updates.

    Example:
        ```python
        def handle_cursor(reader: MessageReader[CursorMoved]) -> None:
            for event in reader:
                print(f"Cursor at: {event.position}")
        ```
    """

    def __init__(
        self, position: Vec2, window: Entity, delta: Vec2 | None = None
    ) -> None: ...
    @property
    def position(self) -> Vec2:
        """Cursor position in window coordinates."""

    @property
    def window(self) -> Entity:
        """Window entity that received the cursor event."""

    @property
    def delta(self) -> Vec2 | None:
        """Change in cursor position since last event, or None if cursor was outside."""


class WindowResized(Message):
    """
    Window resize event message.

    Contains information about window size changes.
    Use with MessageReader to receive window resize events.

    Example:
        ```python
        def handle_resize(reader: MessageReader[WindowResized]) -> None:
            for event in reader:
                print(f"Window resized to {event.width} x {event.height}")
        ```
    """

    def __init__(self, width: float, height: float, window: Entity) -> None: ...
    @property
    def width(self) -> float:
        """Window width in pixels."""

    @property
    def height(self) -> float:
        """Window height in pixels."""

    @property
    def window(self) -> Entity:
        """Window entity that was resized."""


class WindowFocused(Message):
    """
    Window focus event message.

    Contains information about window focus changes.
    Use with MessageReader to receive window focus events.

    Example:
        ```python
        def handle_focus(reader: MessageReader[WindowFocused]) -> None:
            for event in reader:
                if event.focused:
                    print("Window gained focus")
                else:
                    print("Window lost focus")
        ```
    """

    def __init__(self, focused: bool, window: Entity) -> None: ...
    @property
    def focused(self) -> bool:
        """Whether the window has focus."""

    @property
    def window(self) -> Entity:
        """Window entity that changed focus."""


class WindowCloseRequested(Message):
    """
    Window close request event message.

    Triggered when the user attempts to close the window.
    Use with MessageReader to receive window close events.

    Example:
        ```python
        def handle_close(reader: MessageReader[WindowCloseRequested]) -> None:
            for event in reader:
                print("Window close requested")
                # Can handle cleanup or prevent closing
        ```
    """

    def __init__(self, window: Entity) -> None: ...
    @property
    def window(self) -> Entity:
        """Window entity that received the close request."""


class RequestRedraw(Message):
    """
    Request to redraw all windows.

    Send this event to force all windows to redraw, even if their control flow
    is set to Wait and there have been no window events.

    Example:
        ```python
        def force_redraw(writer: MessageWriter[RequestRedraw]) -> None:
            writer.write(RequestRedraw())
        ```
    """

    def __init__(self) -> None: ...


class WindowEvent:
    """Window event union type (read-only).

    A unified event type that wraps all window-related events, allowing you to
    receive all events through a single MessageReader and pattern match on types.

    This is a PyO3 enum - use Python's pattern matching to handle different event types.

    Supported variants:
        - AppLifecycle(lifecycle: AppLifecycle)
        - CursorEntered(window: Entity)
        - CursorLeft(window: Entity)
        - CursorMoved(position: Vec2, window: Entity, delta: Vec2 | None)
        - FileDragAndDrop(window: Entity, path: str | None)
        - Ime(window: Entity)
        - RequestRedraw()
        - WindowCloseRequested(window: Entity)
        - WindowFocused(focused: bool, window: Entity)
        - WindowResized(width: float, height: float, window: Entity)
        - MouseButtonInput(button: MouseButton, state: ButtonState, window: Entity)
        - MouseMotion(delta: Vec2)
        - MouseWheel(unit: MouseScrollUnit, x: float, y: float, window: Entity)
        - PinchGesture(value: float)
        - RotationGesture(value: float)
        - DoubleTapGesture()
        - PanGesture(x: float, y: float)
        - TouchInput(phase: TouchPhase, position: Vec2, id: int, window: Entity, force: float | None)
        - KeyboardFocusLost()
        - WindowCreated(window: Entity)
        - WindowDestroyed(window: Entity)
        - WindowMoved(position: IVec2, window: Entity)
        - WindowOccluded(occluded: bool, window: Entity)
        - WindowScaleFactorChanged(scale_factor: float, window: Entity)
        - WindowBackendScaleFactorChanged(scale_factor: float, window: Entity)
        - WindowThemeChanged(theme: WindowTheme, window: Entity)
        - KeyboardInput(value: KeyboardInput)

    Example:
        ```python
        from pybevy.ecs import MessageReader
        from pybevy.window import WindowEvent

        def handle_window_events(events: MessageReader[WindowEvent]) -> None:
            for event in events:
                match event:
                    case WindowEvent.MouseButtonInput(button=b, state=s, window=w):
                        print(f"Mouse button: {b}")
                    case WindowEvent.CursorMoved(position=pos, window=w):
                        print(f"Cursor at: {pos}")
                    case WindowEvent.WindowResized(width=w, height=h):
                        print(f"Window resized to: {w}x{h}")
        ```
    """

    class AppLifecycle(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["lifecycle"]]]
        lifecycle: AppLifecycle
        def __init__(self, lifecycle: AppLifecycle) -> None: ...

    class CursorEntered(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["window"]]]
        window: Entity
        def __init__(self, window: Entity) -> None: ...

    class CursorLeft(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["window"]]]
        window: Entity
        def __init__(self, window: Entity) -> None: ...

    class CursorMoved(WindowEvent):
        __match_args__: ClassVar[
            tuple[Literal["position"], Literal["window"], Literal["delta"]]
        ]
        position: Vec2
        window: Entity
        delta: Vec2 | None
        def __init__(
            self, position: Vec2, window: Entity, delta: Vec2 | None = None
        ) -> None: ...

    class FileDragAndDrop(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: FileDragAndDrop
        def __init__(self, value: FileDragAndDrop) -> None: ...

    class Ime(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: Ime
        def __init__(self, value: Ime) -> None: ...

    class RequestRedraw(WindowEvent):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class WindowCloseRequested(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["window"]]]
        window: Entity
        def __init__(self, window: Entity) -> None: ...

    class WindowFocused(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["focused"], Literal["window"]]]
        focused: bool
        window: Entity
        def __init__(self, focused: bool, window: Entity) -> None: ...

    class WindowResized(WindowEvent):
        __match_args__: ClassVar[
            tuple[Literal["width"], Literal["height"], Literal["window"]]
        ]
        width: float
        height: float
        window: Entity
        def __init__(self, width: float, height: float, window: Entity) -> None: ...

    class MouseButtonInput(WindowEvent):
        __match_args__: ClassVar[
            tuple[Literal["button"], Literal["state"], Literal["window"]]
        ]
        button: MouseButton
        state: ButtonState
        window: Entity
        def __init__(
            self, button: MouseButton, state: ButtonState, window: Entity
        ) -> None: ...

    class MouseMotion(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["delta"]]]
        delta: Vec2
        def __init__(self, delta: Vec2) -> None: ...

    class MouseWheel(WindowEvent):
        __match_args__: ClassVar[
            tuple[Literal["unit"], Literal["x"], Literal["y"], Literal["window"]]
        ]
        unit: MouseScrollUnit
        x: float
        y: float
        window: Entity
        def __init__(
            self, unit: MouseScrollUnit, x: float, y: float, window: Entity
        ) -> None: ...

    class PinchGesture(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float) -> None: ...

    class RotationGesture(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float) -> None: ...

    class DoubleTapGesture(WindowEvent):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PanGesture(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["x"], Literal["y"]]]
        x: float
        y: float
        def __init__(self, x: float, y: float) -> None: ...

    class TouchInput(WindowEvent):
        __match_args__: ClassVar[
            tuple[
                Literal["phase"],
                Literal["position"],
                Literal["id"],
                Literal["window"],
                Literal["force"],
            ]
        ]
        phase: TouchPhase
        position: Vec2
        id: int
        window: Entity
        force: float | None
        def __init__(
            self,
            phase: TouchPhase,
            position: Vec2,
            id: int,
            window: Entity,
            force: float | None = None,
        ) -> None: ...

    class KeyboardFocusLost(WindowEvent):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class WindowCreated(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["window"]]]
        window: Entity
        def __init__(self, window: Entity) -> None: ...

    class WindowDestroyed(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["window"]]]
        window: Entity
        def __init__(self, window: Entity) -> None: ...

    class WindowMoved(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["position"], Literal["window"]]]
        position: IVec2
        window: Entity
        def __init__(self, position: IVec2, window: Entity) -> None: ...

    class WindowOccluded(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["occluded"], Literal["window"]]]
        occluded: bool
        window: Entity
        def __init__(self, occluded: bool, window: Entity) -> None: ...

    class WindowScaleFactorChanged(WindowEvent):
        __match_args__: ClassVar[
            tuple[Literal["scale_factor"], Literal["window"]]
        ]
        scale_factor: float
        window: Entity
        def __init__(self, scale_factor: float, window: Entity) -> None: ...

    class WindowBackendScaleFactorChanged(WindowEvent):
        __match_args__: ClassVar[
            tuple[Literal["scale_factor"], Literal["window"]]
        ]
        scale_factor: float
        window: Entity
        def __init__(self, scale_factor: float, window: Entity) -> None: ...

    class WindowThemeChanged(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["theme"], Literal["window"]]]
        theme: WindowTheme
        window: Entity
        def __init__(self, theme: WindowTheme, window: Entity) -> None: ...

    class KeyboardInput(WindowEvent):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: KeyboardInput
        def __init__(self, value: KeyboardInput) -> None: ...
