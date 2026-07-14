from collections.abc import Callable
from enum import Enum, auto
from typing import Any, ClassVar, TypeVar

from pybevy.ecs import (
    ConditionalSystem,
    Message,
    MessageTypeVar,
    OnEnterSchedule,
    OnExitSchedule,
    OnTransitionSchedule,
    Resource,
    ResourceType,
)

type SystemFn = Callable[..., None]
type SystemFns = tuple[SystemFn, ...] | SystemFn | ChainedSystems | ConditionalSystem

T = TypeVar("T")

class ChainedSystems:
    """Wrapper for a sequence of systems that should be executed sequentially."""
    def __init__(self, *systems: SystemFn) -> None: ...

def chain(*systems: SystemFn) -> ChainedSystems:
    """Chain multiple systems to run sequentially.

    Chained systems will execute in the order they are provided, with each
    system completing before the next one starts.

    Args:
        *systems: Variable number of system functions to chain together

    Returns:
        ChainedSystems object that can be passed to add_systems()

    Example:
        ```python
        from pybevy import chain

        def system1(): pass
        def system2(): pass
        def system3(): pass

        # These systems will run in order: system1 -> system2 -> system3
        app.add_systems(Update, chain(system1, system2, system3))
        ```

    Note:
        Currently supports chaining up to 4 systems.
    """

def _test_get_app_count() -> int:
    """TEST ONLY: Get the count of Apps currently in thread-local storage.

    Used to verify that atexit cleanup works correctly.
    """

def _test_force_cleanup() -> None:
    """TEST ONLY: Force immediate cleanup of all Apps in thread-local storage.

    This simulates the cleanup that should happen via atexit handler.
    Used to test that cleanup works correctly with Python resources.
    """

class Stage(Enum):
    """Schedule labels for system execution ordering.

    PyBevy provides 14 schedules that run in a specific order each frame:

    **Startup Schedules** (run once):
    - PreStartup: Before main startup
    - Startup: Main initialization
    - PostStartup: After main startup

    **Frame Schedules** (run every frame):
    - Main: Outermost schedule
    - First: First frame logic
    - PreUpdate: Input handling, before game logic
    - SimTick: Simulation tick (RL workloads, runs between PreUpdate and Update)
    - Update: Main game logic
    - PostUpdate: Rendering prep, after game logic
    - Last: Final frame cleanup

    **Fixed Update Schedules** (run at fixed timestep):
    - FixedFirst: Before fixed update
    - FixedPreUpdate: Fixed pre-update
    - FixedUpdate: Main fixed update (physics, etc.)
    - FixedPostUpdate: Fixed post-update
    - FixedLast: After fixed update
    """

    Startup = auto()
    Update = auto()
    Last = auto()
    FixedUpdate = auto()
    Main = auto()
    First = auto()
    PreUpdate = auto()
    PostUpdate = auto()
    PreStartup = auto()
    PostStartup = auto()
    FixedFirst = auto()
    FixedPreUpdate = auto()
    FixedPostUpdate = auto()
    FixedLast = auto()
    SimTick = auto()

# Convenience aliases for schedule labels
Startup = Stage.Startup
Update = Stage.Update
Last = Stage.Last
FixedUpdate = Stage.FixedUpdate
Main = Stage.Main
First = Stage.First
PreUpdate = Stage.PreUpdate
PostUpdate = Stage.PostUpdate
PreStartup = Stage.PreStartup
PostStartup = Stage.PostStartup
FixedFirst = Stage.FixedFirst
FixedPreUpdate = Stage.FixedPreUpdate
FixedPostUpdate = Stage.FixedPostUpdate
FixedLast = Stage.FixedLast
SimTick = Stage.SimTick

class Plugin:
    """Base class for individual Bevy plugins.

    Plugins encapsulate reusable app configuration including systems, resources,
    and other plugins. The `build(self, app)` method is called by `add_plugins()`.

    IMPORTANT: Custom plugins MUST use BOTH the @plugin decorator AND
    inherit from Plugin.

    Example:
        ```python
        from pybevy.decorators import plugin

        @plugin  # Required decorator
        class MyGamePlugin(Plugin):  # MUST inherit from Plugin
            def build(self, app: App) -> None:
                app.add_systems(Startup, setup)
                app.add_systems(Update, game_loop)
                app.insert_resource(GameState())

        # Add plugin to app
        app.add_plugins(MyGamePlugin())
        ```
    """
    def build(self, app: App) -> None: ...

class PluginGroup:
    """Base class for plugin groups (collections of plugins).

    Plugin groups implement `build() -> PluginGroupBuilder` which returns a builder
    for configuring which plugins to include/exclude.

    This matches Bevy's `PluginGroup` trait (separate from `Plugin` trait).

    Example:
        ```python
        # DefaultPlugins is a PluginGroup
        app.add_plugins(DefaultPlugins)

        # Use build() for configuration
        app.add_plugins(DefaultPlugins.build().disable(AudioPlugin))
        ```
    """
    def build(self) -> PluginGroupBuilder:
        """Build the plugin group, returning a builder for configuration."""

class PluginGroupBuilder(PluginGroup):
    """Builder for configuring plugin groups.

    Provides methods for configuring, disabling, and adding plugins.
    All methods return a new builder instance (immutable pattern).

    This is returned by `PluginGroup.build()` and allows fine-grained
    control over which plugins are included.
    """

    def set(self, plugin: Plugin) -> PluginGroupBuilder:
        """Replace a plugin in the group.

        Args:
            plugin: Configured plugin instance to replace the default

        Returns:
            New builder with the plugin configured

        Example:
            >>> from pybevy.window import WindowPlugin, Window
            >>> builder.set(WindowPlugin(primary_window=Window(title="Game")))
        """

    def disable(self, plugin_type: type[Plugin]) -> PluginGroupBuilder:
        """Disable a specific plugin from the group.

        Args:
            plugin_type: Plugin class to disable

        Returns:
            New builder with the plugin disabled

        Example:
            >>> builder.disable(AudioPlugin)
        """

    def add(self, plugin: Plugin) -> PluginGroupBuilder:
        """Add a plugin to the end of the group.

        Args:
            plugin: Plugin instance to add

        Returns:
            New builder with the plugin added
        """

    def add_before(self, target: type[Plugin], plugin: Plugin) -> PluginGroupBuilder:
        """Add a plugin before the target plugin.

        Args:
            target: Plugin class to insert before
            plugin: Plugin instance to add

        Returns:
            New builder with the plugin added
        """

    def add_after(self, target: type[Plugin], plugin: Plugin) -> PluginGroupBuilder:
        """Add a plugin after the target plugin.

        Args:
            target: Plugin class to insert after
            plugin: Plugin instance to add

        Returns:
            New builder with the plugin added
        """

    def enable(self, plugin_type: type[Plugin]) -> PluginGroupBuilder:
        """Re-enable a previously disabled plugin.

        Args:
            plugin_type: Plugin class to enable

        Returns:
            New builder with the plugin enabled
        """

    def build(self, app: App) -> None:  # type: ignore[override]
        """Build and apply all plugins in the group to the app.

        This is called by `add_plugins()` to apply the configured plugins.
        """

class DefaultPlugins(PluginGroup):
    """Default plugin group for typical PyBevy applications.

    Includes window, rendering, input, assets, and other core functionality.
    This is the most common way to set up a PyBevy application.

    Example:
        ```python
        # Simple usage - adds all default plugins
        app.add_plugins(DefaultPlugins)

        # Configure a specific plugin
        app.add_plugins(DefaultPlugins.set(WindowPlugin(primary_window=...)))

        # Disable specific plugins
        app.add_plugins(DefaultPlugins.build().disable(AudioPlugin))
        ```
    """

    def __init__(self) -> None: ...

    def set(self, plugin: Plugin) -> PluginGroupBuilder:
        """Configure a specific plugin in the group.

        Convenience method equivalent to .build().set(plugin).

        Args:
            plugin: Configured plugin instance to replace the default

        Returns:
            PluginGroupBuilder with the configuration applied

        Example:
            >>> from pybevy.window import WindowPlugin, Window
            >>> DefaultPlugins.set(WindowPlugin(primary_window=Window(title="Game")))
        """

    def build(self) -> PluginGroupBuilder:
        """Get builder for advanced configuration.

        Returns:
            PluginGroupBuilder for chaining configuration methods

        Example:
            >>> DefaultPlugins.build().disable(AudioPlugin)
        """

    def _apply_to_app(self, app: App) -> None:
        """Internal method to apply the default plugins to the app.

        This is called by add_plugins() and should not be called directly.
        """

class MinimalPlugins(Plugin):
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class TaskPoolPlugin(Plugin):
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class HotReloadPlugin(Plugin):
    """Plugin to enable hot reload functionality.

    This plugin sets up the hot reload system, including:
    - HotReloadControl resource for requesting reloads
    - HotReloadGeneration resource for tracking reload generations
    - F5 key handler for triggering reloads
    - Systems for processing reload requests

    Example:
        ```python
        from pybevy.app import App, HotReloadPlugin, ScheduleRunnerPlugin

        app = App()
        app.add_plugins(ScheduleRunnerPlugin.run_once())
        app.add_plugins(HotReloadPlugin())  # Enable hot reload
        ```

    Note: Without this plugin, HotReloadControl resource will not be available.
    """
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class AppReloadState:
    """State object for hot reload functionality.

    Used by CLI dev/watch mode to trigger hot reloads when files change.
    """
    def set_pending_reload(self) -> None:
        """Request a hot reload on the next frame."""
    def set_pending_partial_reload(self) -> None:
        """Request a partial hot reload (Update/Last systems only)."""
    def trigger_reload_if_needed(self, default_mode_is_partial: bool) -> None:
        """Trigger reload if not already pending, using current default mode."""
    def get_default_mode(self) -> str:
        """Get the current default reload mode ('Full' or 'Partial')."""
    def is_partial_reload(self) -> bool:
        """Check if the next reload will be in partial mode."""

class HotReloadControl(Resource):
    """Control hot reload behavior from within systems.

    This resource allows systems to request reloads dynamically, such as when
    the user presses F5 for a full reload.

    Example:
        ```python
        def handle_f5(input: ButtonInput, world: World) -> None:
            if input.just_pressed(KeyCode.F5):
                control = world.resource(HotReloadControl)
                control.request_full_reload()
        ```

    Note: This resource is only available when hot reload is enabled
    (i.e., when running with `pybevy dev` or `pybevy watch`).
    """

    def request_full_reload(self) -> None:
        """Request a full reload on the next file change.

        Full reload will:
        - Despawn all user-created entities
        - Clear resources
        - Reload all systems including Startup
        - Reset the application state
        """

    def request_partial_reload(self) -> None:
        """Request a partial reload on the next file change (default).

        Partial reload will:
        - Keep entities and resources intact
        - Only update Update/Last systems
        - Preserve application state
        """

    def is_enabled(self) -> bool:
        """Check if hot reload is currently enabled.

        Returns:
            True if hot reload is active, False otherwise
        """

    def generation(self) -> int:
        """Get the current generation number.

        Each reload increments the generation counter.

        Returns:
            Current generation number (starts at 0)
        """

class App:
    def __init__(self) -> None: ...
    def add_systems(self, stage: Stage | OnEnterSchedule | OnExitSchedule | OnTransitionSchedule, *systems: SystemFns) -> App: ...
    def initialize(self) -> None: ...
    def finish(self) -> None: ...
    def update(self) -> None: ...
    def run(self) -> None: ...
    def _mark_entrypoint(self) -> None: ...
    def world(self, callback: Callable[..., None]) -> None: ...
    def run_system_once(self, func: SystemFn) -> None:
        """Run a system function once immediately on this app's world."""
    def _run_systems_once(self, *funcs: SystemFn) -> None:
        """Run multiple system functions once immediately (PyBevy-specific)."""
    def init_resource(self, resource: type[ResourceType]) -> App: ...
    def insert_resource(self, resource: Resource) -> App: ...
    def init_state(self, state_type: type) -> App:
        """Initialize a state machine with the first enum variant as default.

        Args:
            state_type: An Enum class decorated with @state

        Returns:
            self for method chaining

        Example:
            @state
            class GameState(Enum):
                MENU = auto()
                IN_GAME = auto()

            app.init_state(GameState)  # Starts in MENU
        """
    def insert_state(self, initial_state: Any) -> App:
        """Initialize a state machine with a specific initial state.

        Args:
            initial_state: An enum value from a @state decorated Enum

        Returns:
            self for method chaining

        Example:
            app.insert_state(GameState.IN_GAME)  # Starts in IN_GAME
        """
    def add_plugins(
        self, *plugins: Plugin | PluginGroup | type[Plugin] | type[PluginGroup] | tuple[Plugin | PluginGroup | type[Plugin] | type[PluginGroup], ...]
    ) -> App: ...
    def add_message(self, message_type: type[MessageTypeVar]) -> App: ...
    def add_observer(self, observer: SystemFn) -> App:
        """Register an observer for an event type.

        The observer will be triggered whenever the event is sent via
        World.trigger() or Commands.trigger().

        Returns self for method chaining. For lifecycle management, use
        World.add_observer() instead to get the observer entity ID.

        Example:
            def on_player_died(trigger: On[PlayerDied]) -> None:
                event = trigger.event()
                print(f"Player {event.player_id} died")

            app.add_observer(on_player_died).add_systems(...)
        """
    def is_plugin_added(self, plugin_type: type[Plugin] | type[PluginGroup]) -> bool: ...
    def should_exit(self) -> AppExit | None:
        """Check if the app should exit.

        Returns the AppExit value if an exit has been requested, None otherwise.
        This allows checking exit status programmatically for conditional logic
        or tests.

        Returns:
            AppExit if exit requested, None if app should continue running

        Example:
            ```python
            def check_quit(app_exit_writer: MessageWriter[AppExit]) -> None:
                # Request exit after some condition
                app_exit_writer.send(AppExit.SUCCESS)

            def verify_exit(app: Res[App]) -> None:
                exit_status = app.should_exit()
                if exit_status and exit_status.is_success():
                    print("App is exiting successfully")
            ```
        """

    def run_schedule(self, stage: Stage) -> None:
        """Run a specific schedule once on the app's world.

        This executes only the systems registered in the given schedule,
        without running the full frame update. Useful for RL simulation
        workloads where SimTick needs to run independently of rendering.

        Args:
            stage: The schedule to run (e.g., SimTick, Update)

        Example:
            ```python
            # Run SimTick independently (without other frame schedules)
            app.run_schedule(SimTick)

            # Run Update schedule independently
            app.run_schedule(Update)
            ```
        """
    def init_schedule(self, label: Stage) -> App:
        """Initialize a schedule and add it to the app.

        Creates an empty schedule with the given label. The schedule can then
        be used with add_systems() to add systems to it.

        Args:
            label: The schedule label (Stage) to initialize

        Returns:
            self for method chaining

        Example:
            ```python
            app.init_schedule(PreUpdate)
            app.add_systems(PreUpdate, handle_input)
            ```

        Note:
            Most schedules are automatically initialized when first used.
            This method is only needed for custom schedules or explicit
            initialization order.
        """

    def configure_sets(self, schedule: Stage, sets: Any) -> None:
        """Configure system set ordering and relationships.

        **STUB**: This method is not yet implemented. Full implementation requires:
        - System set type wrappers
        - Before/after relationship tracking
        - Integration with DynamicSystem

        For now, use schedule labels (First, PreUpdate, Update, PostUpdate, Last)
        to control execution order at a coarse granularity.

        Args:
            schedule: The schedule to configure sets in
            sets: System set configuration (not yet supported)

        Raises:
            RuntimeError: Always raises - method not yet implemented

        Future usage will be:
            ```python
            # This will work when implemented:
            app.configure_sets(Update, MySet.before(OtherSet))
            ```

        Workaround:
            ```python
            # Use different schedules for ordering:
            app.add_systems(PreUpdate, input_systems)   # Runs first
            app.add_systems(Update, game_logic)         # Runs second
            app.add_systems(PostUpdate, render_prep)    # Runs third
            ```
        """

    def cleanup(self) -> None:
        """Clean up app resources."""
    def clear_scene(self) -> None:
        """Clear the scene by despawning all entities and clearing custom resources.

        This is similar to hot reload's Full mode but without reloading systems.
        Useful for JupyBevy to reset the scene when creating a new instance.

        Preserves:
        - Built-in Bevy resources (Time, AssetServer, etc.)
        - RenderDevice and render infrastructure
        - Plugin state

        Clears:
        - All entities
        - Custom Python resources
        """

    # Hot reload support (for CLI dev/watch mode)
    @property
    def _state(self) -> AppReloadState: ...
    def _set_hot_reload_loader(
        self, loader: Callable[[], Callable[[], App]]
    ) -> App: ...


class Schedule:
    def run_stage(self, stage: Stage) -> None: ...

class RunMode:
    """Determines the method used to run an App's schedule.

    Used with ScheduleRunnerPlugin to control how the app executes.

    Attributes:
        Loop: Run the schedule repeatedly (default behavior)
        Once: Run the schedule exactly once then exit

    Examples:
        ```python
        # Run once (most common for tests)
        RunMode.Once()

        # Run in a loop without waiting
        RunMode.Loop()

        # Run in a loop with 16ms wait between frames (~60 FPS)
        RunMode.Loop(wait=16)
        ```
    """

    @staticmethod
    def Loop(wait: int | None = None) -> RunMode:
        """Run the schedule repeatedly in a loop.

        Args:
            wait: Optional wait time in milliseconds between schedule executions.
                  If None, the schedule runs continuously without waiting.
                  Use this to cap the frame rate (e.g., 16ms for ~60 FPS).

        Returns:
            RunMode configured for continuous execution

        Example:
            ```python
            # No wait - run as fast as possible
            mode = RunMode.Loop()

            # Wait 16ms between frames (~60 FPS)
            mode = RunMode.Loop(wait=16)
            ```
        """

    @staticmethod
    def Once() -> RunMode:
        """Run the schedule exactly once then exit.

        This is the preferred mode for testing, as it runs all schedules
        (Startup, Update, etc.) through one complete cycle then stops.

        Returns:
            RunMode configured for single execution

        Example:
            ```python
            # Typical test setup
            app = App()
            app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
            app.add_systems(Update, my_test_system)
            app.run()  # Runs once and exits
            ```
        """

class ScheduleRunnerPlugin(Plugin):
    """Configures an App to run its schedule according to a given RunMode.

    This plugin is essential for headless applications and testing. It controls
    the app's execution loop, determining whether it runs once or continuously.

    **Plugin Groups:**
    - Included in MinimalPlugins
    - NOT included in DefaultPlugins (which uses WinitPlugin's event loop instead)

    **Testing Usage:**
    For tests, always use `ScheduleRunnerPlugin(RunMode.Once())` instead of calling
    `app.update()` directly. This ensures the full schedule lifecycle runs correctly.

    Args:
        run_mode: Controls execution behavior (default: RunMode.Loop())

    Examples:
        ```python
        # Testing - run once and exit
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
        app.add_systems(Startup, setup)
        app.add_systems(Update, game_logic)
        app.run()  # Runs full schedule once

        # Headless server - run continuously
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(RunMode.Loop()))
        app.add_systems(Update, server_tick)
        app.run()  # Runs forever

        # Frame-limited loop
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(RunMode.Loop(wait=16)))
        app.run()  # Runs at ~60 FPS
        ```

    Notes:
        - For graphical applications, use DefaultPlugins instead (includes WinitPlugin)
        - For tests, prefer RunMode.Once() over app.update()
        - Startup systems run before the first Update when using RunMode.Once()
    """

    def __init__(self, run_mode: RunMode = ...) -> None:
        """Create a ScheduleRunnerPlugin with the specified run mode.

        Args:
            run_mode: Execution mode (default: RunMode.Loop())
        """

    def build(self, app: App) -> None:
        """Build and apply the plugin to the app."""

    @staticmethod
    def run_once() -> ScheduleRunnerPlugin:
        """Create a plugin configured to run the schedule once.

        Convenience method equivalent to `ScheduleRunnerPlugin(RunMode.Once())`.
        This is the preferred method for test setup.

        Returns:
            ScheduleRunnerPlugin configured for single execution

        Example:
            ```python
            def test_my_system() -> None:
                app = App().add_plugins(ScheduleRunnerPlugin.run_once())
                app.add_systems(Update, my_system)
                app.run()
            ```
        """

    @staticmethod
    def run_loop(wait_duration: int | None = None) -> ScheduleRunnerPlugin:
        """Create a plugin configured to run the schedule in a loop.

        Convenience method equivalent to `ScheduleRunnerPlugin(RunMode.Loop(wait=...))`.

        Args:
            wait_duration: Optional wait time in milliseconds between executions.
                          None means run as fast as possible.

        Returns:
            ScheduleRunnerPlugin configured for continuous execution

        Example:
            ```python
            # Headless server running as fast as possible
            app = App().add_plugins(ScheduleRunnerPlugin.run_loop())

            # Server with 60 FPS cap
            app = App().add_plugins(ScheduleRunnerPlugin.run_loop(wait_duration=16))
            ```
        """

class AppExit(Message):
    SUCCESS: ClassVar[AppExit]

    def __init__(self) -> None: ...
    @staticmethod
    def error(code: int) -> AppExit: ...
    def is_success(self) -> bool: ...
    def is_error(self) -> bool: ...
