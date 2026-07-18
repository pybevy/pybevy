"""Winit integration for PyBevy.

This module provides PyBevy's integration with the winit windowing library,
which handles cross-platform window creation, event loops, and OS input events.

The `WinitPlugin` is the core plugin that enables graphical applications by
creating windows and processing events from the operating system.

See Also:
    - `pybevy.window.WindowPlugin`: Configure window properties
    - `pybevy.app.DefaultPlugins`: Includes WinitPlugin by default
    - `pybevy.app.ScheduleRunnerPlugin`: Alternative for headless applications
"""

from typing import ClassVar, Literal

from pybevy.app import App, Plugin

class UpdateMode:
    """Controls how frequently the application updates.

    Construct exact variants directly, or use Bevy's convenience constructors:

    Examples:
        ```python
        UpdateMode.Continuous()           # Full speed rendering
        UpdateMode.reactive(wait=0.1)     # Reactive, all events, 100ms wait
        UpdateMode.reactive_low_power(wait=0.2)  # Low power mode
        ```
    """

    class Continuous(UpdateMode):
        __match_args__: ClassVar[tuple[()]]

        def __init__(self) -> None: ...

    class Reactive(UpdateMode):
        __match_args__: ClassVar[
            tuple[
                Literal["wait"],
                Literal["react_to_device_events"],
                Literal["react_to_user_events"],
                Literal["react_to_window_events"],
            ]
        ]
        wait: float
        react_to_device_events: bool
        react_to_user_events: bool
        react_to_window_events: bool

        def __init__(
            self,
            wait: float,
            react_to_device_events: bool,
            react_to_user_events: bool,
            react_to_window_events: bool,
        ) -> None: ...

    @staticmethod
    def reactive(wait: float = 1.0) -> UpdateMode.Reactive:
        """Reactive mode - updates in response to events or after `wait` seconds.

        Reacts to all event types (window, device, and user events).

        Args:
            wait: Time in seconds between updates (default: 1.0)
        """

    @staticmethod
    def reactive_low_power(wait: float = 1.0) -> UpdateMode.Reactive:
        """Low power reactive mode - only reacts to window and user events.

        Unlike `reactive()`, this ignores device events like general mouse
        movement (only reacts when the cursor is over a window). This can
        greatly reduce power consumption.

        Args:
            wait: Time in seconds between updates (default: 1.0)
        """

class WinitSettings:
    """Settings for the WinitPlugin controlling event loop update frequency.

    Use presets for common configurations, or construct with custom modes:

    Examples:
        ```python
        # Presets
        WinitSettings.game()          # Default for games (continuous when focused)
        WinitSettings.desktop_app()   # Default for desktop apps (reactive)
        WinitSettings.continuous()    # Always max speed

        # Custom
        WinitSettings(
            focused_mode=UpdateMode.reactive(wait=0.05),
            unfocused_mode=UpdateMode.reactive_low_power(wait=0.5)
        )
        ```
    """

    def __init__(
        self,
        focused_mode: UpdateMode | None = None,
        unfocused_mode: UpdateMode | None = None,
    ) -> None:
        """Create WinitSettings with custom update modes.

        Args:
            focused_mode: Update mode when window has focus (default: Continuous)
            unfocused_mode: Update mode when window is unfocused (default: reactive_low_power at 60Hz)
        """

    @staticmethod
    def game() -> WinitSettings:
        """Default settings for games.

        Continuous rendering when focused, reactive low-power at 60Hz when unfocused.
        """

    @staticmethod
    def desktop_app() -> WinitSettings:
        """Default settings for desktop applications.

        Reactive rendering (5s wait) when focused, reactive low-power (60s wait)
        when unfocused. Use for tools, editors, and apps that don't need continuous
        rendering.
        """

    @staticmethod
    def continuous() -> WinitSettings:
        """Maximum speed rendering in both focused and unfocused states."""

    @property
    def focused_mode(self) -> UpdateMode:
        """The update mode when the window has focus."""

    @property
    def unfocused_mode(self) -> UpdateMode:
        """The update mode when the window is unfocused."""

class WinitPlugin(Plugin):
    """Uses winit to create and manage windows, and receive window and input events.

    This plugin integrates the winit windowing library with Bevy, providing
    cross-platform window management and event handling.

    Examples:
        ```python
        from pybevy import DefaultPlugins, WinitPlugin, WinitSettings

        # Default (game mode)
        app.add_plugins(DefaultPlugins())

        # Desktop app mode - reactive rendering
        app.add_plugins(
            DefaultPlugins().set(WinitPlugin(
                settings=WinitSettings.desktop_app()
            ))
        )

        # Custom settings
        app.add_plugins(
            DefaultPlugins().set(WinitPlugin(
                settings=WinitSettings(
                    focused_mode=UpdateMode.reactive(wait=0.05),
                    unfocused_mode=UpdateMode.reactive_low_power(wait=0.5)
                )
            ))
        )
        ```

    See Also:
        - `WindowPlugin`: Configure window properties and behavior
        - `ScheduleRunnerPlugin`: Alternative for headless execution
        - `WinitSettings`: Configure update frequency
    """

    def __init__(self, settings: WinitSettings | None = None) -> None:
        """Create a new WinitPlugin.

        Args:
            settings: Optional WinitSettings to control event loop update frequency.
                If None, uses default game settings (continuous when focused).
        """

    def build(self, app: App) -> None:
        """Configure the app with winit integration.

        This method is called automatically when the plugin is added to the app.
        """
