from collections.abc import Callable
from typing import Any, ClassVar, Final, Literal, TypeVar

from pybevy.ecs import (
    ConditionalSystem,
    Message,
    MessageTypeVar,
    OnEnterSchedule,
    OnExitSchedule,
    OnTransitionSchedule,
    Resource,
    ResourceType,
    SystemConfig,
    SystemSet,
    SystemSetConfig,
    SystemSetEnum,
)

type SystemFn = Callable[..., object]
type SystemFns = (
    tuple[SystemFn | ConditionalSystem | SystemConfig, ...]
    | SystemFn
    | ChainedSystems
    | ConditionalSystem
    | SystemConfig
)
type SystemChainItem = SystemFn | ConditionalSystem | SystemConfig
type SystemSetChainItem = SystemSet | SystemSetEnum | SystemSetConfig

T = TypeVar("T")

AnimationSystems: Final[SystemSet]

class ChainedSystems:
    """Wrapper for a sequence of systems that should be executed sequentially."""
    def __init__(self, *systems: SystemChainItem) -> None: ...

class ChainedSystemSets:
    """Wrapper for a sequence of system sets configured in order."""
    def __init__(self, *sets: SystemSetChainItem) -> None: ...

def _test_get_app_count() -> int:
    """TEST ONLY: Get the count of Apps currently in thread-local storage.

    Used to verify that atexit cleanup works correctly.
    """

def _test_force_cleanup() -> None:
    """TEST ONLY: Force immediate cleanup of all Apps in thread-local storage.

    This simulates the cleanup that should happen via atexit handler.
    Used to test that cleanup works correctly with Python resources.
    """

class Stage:
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

    Startup: Stage
    Update: Stage
    Last: Stage
    FixedUpdate: Stage
    Main: Stage
    First: Stage
    PreUpdate: Stage
    PostUpdate: Stage
    PreStartup: Stage
    PostStartup: Stage
    FixedFirst: Stage
    FixedPreUpdate: Stage
    FixedPostUpdate: Stage
    FixedLast: Stage
    SimTick: Stage

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
    @property
    def __pybevy_plugin_key__(self) -> str | None:
        """Stable key for multiple instances, or None for class-only identity."""
    def build(self, app: App) -> None: ...

class PluginGroup:
    """Base class for plugin groups (collections of plugins).

    Plugin groups implement `build() -> PluginGroupBuilder` which returns a builder
    for configuring which plugins to include/exclude.

    `App.add_plugins()` calls `build()` and applies the returned builder.
    Python subclasses use the same contract as native groups.

    Example:
        ```python
        # DefaultPlugins is a PluginGroup
        app.add_plugins(DefaultPlugins)

        # Use build() for configuration
        app.add_plugins(DefaultPlugins().build().disable(AudioPlugin))
        ```
    """
    @classmethod
    def name(cls) -> str:
        """Return the group name used by start()."""
    def build(self) -> PluginGroupBuilder:
        """Build the plugin group, returning a builder for configuration."""
    def set(self, plugin: Plugin) -> PluginGroupBuilder:
        """Build the group and replace an existing member's configuration."""

class PluginGroupBuilder(PluginGroup):
    """An ordered plugin collection with reusable immutable configuration.

    Configuration methods return new builders. Native members use Bevy type
    identity; Python members use their class identity. Adding an existing type
    replaces its value, enables it, and moves it to the requested position.
    """

    @staticmethod
    def start(group_type: type[PluginGroup]) -> PluginGroupBuilder:
        """Start an empty collection named for the given group class."""
    def contains(self, plugin_type: type[Plugin]) -> bool:
        """Check membership, including disabled members."""
    def enabled(self, plugin_type: type[Plugin]) -> bool:
        """Check whether a member exists and is enabled."""
    def set(self, plugin: Plugin) -> PluginGroupBuilder:
        """Replace an existing value, preserving its position and enabled state."""
    def disable(self, plugin_type: type[Plugin]) -> PluginGroupBuilder:
        """Disable an existing member, preserving its position."""
    def enable(self, plugin_type: type[Plugin]) -> PluginGroupBuilder:
        """Enable an existing member."""
    def add(self, plugin: Plugin) -> PluginGroupBuilder:
        """Replace or append a member, enabling it."""
    def add_before(self, target: type[Plugin], plugin: Plugin) -> PluginGroupBuilder:
        """Replace or insert a member immediately before an existing target."""
    def add_after(self, target: type[Plugin], plugin: Plugin) -> PluginGroupBuilder:
        """Replace or insert a member immediately after an existing target."""
    def add_group(self, group: PluginGroup) -> PluginGroupBuilder:
        """Merge members at the end, preserving their enabled states."""
    def build(self) -> PluginGroupBuilder:
        """Return this same builder."""
    def finish(self, app: App) -> None:
        """Apply enabled members in order. Installed custom/minimal members are skipped.
        Repeated native DefaultPlugins seed members can raise RuntimeError."""

class DefaultPlugins(PluginGroup):
    """Default plugin group for typical PyBevy applications.

    Includes window, rendering, input, assets, and other core functionality.
    This is the most common way to set up a PyBevy application.

    Example:
        ```python
        # Simple usage - adds all default plugins
        app.add_plugins(DefaultPlugins)

        # Configure a specific plugin
        app.add_plugins(DefaultPlugins().set(WindowPlugin(primary_window=...)))

        # Disable specific plugins
        app.add_plugins(DefaultPlugins().build().disable(AudioPlugin))
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
            >>> DefaultPlugins().set(WindowPlugin(primary_window=Window(title="Game")))
        """

    def build(self) -> PluginGroupBuilder:
        """Get builder for advanced configuration.

        Returns:
            PluginGroupBuilder for chaining configuration methods

        Example:
            >>> DefaultPlugins().build().disable(AudioPlugin)
        """

    def finish(self, app: App) -> None:
        """Apply defaults without consuming this group.

        Repeating on the same App raises RuntimeError for duplicate native plugins.
        """

class MinimalPlugins(PluginGroup):
    """Minimal plugin group for headless applications and tests.

    Installs the task pool, frame count, time, and schedule runner plugins.

    Example:
        ```python
        app.add_plugins(MinimalPlugins)
        ```
    """

    def __init__(self) -> None: ...
    def build(self) -> PluginGroupBuilder:
        """Return a configurable builder containing the minimal group members."""
    def finish(self, app: App) -> None:
        """Apply minimal plugins, skipping members already installed on the App."""

class TaskPoolThreadAssignmentPolicy:
    """Bevy's per-pool thread assignment policy.

    The native worker-thread lifecycle callbacks are intentionally unavailable;
    invoking retained Python callables on Bevy worker threads requires a separate
    free-threading and exception-delivery contract.
    """

    def __init__(
        self, *, min_threads: int, max_threads: int, percent: float
    ) -> None: ...
    @property
    def min_threads(self) -> int: ...
    @min_threads.setter
    def min_threads(self, value: int) -> None: ...
    @property
    def max_threads(self) -> int: ...
    @max_threads.setter
    def max_threads(self, value: int) -> None: ...
    @property
    def percent(self) -> float: ...
    @percent.setter
    def percent(self, value: float) -> None: ...

class TaskPoolOptions:
    """Configure how Bevy divides available threads among its global pools."""

    def __init__(
        self,
        *,
        min_total_threads: int = 1,
        max_total_threads: int = ...,
        io: TaskPoolThreadAssignmentPolicy | None = None,
        async_compute: TaskPoolThreadAssignmentPolicy | None = None,
        compute: TaskPoolThreadAssignmentPolicy | None = None,
    ) -> None: ...
    @staticmethod
    def with_num_threads(thread_count: int) -> TaskPoolOptions: ...
    @property
    def min_total_threads(self) -> int: ...
    @min_total_threads.setter
    def min_total_threads(self, value: int) -> None: ...
    @property
    def max_total_threads(self) -> int: ...
    @max_total_threads.setter
    def max_total_threads(self, value: int) -> None: ...
    @property
    def io(self) -> TaskPoolThreadAssignmentPolicy: ...
    @io.setter
    def io(self, value: TaskPoolThreadAssignmentPolicy) -> None: ...
    @property
    def async_compute(self) -> TaskPoolThreadAssignmentPolicy: ...
    @async_compute.setter
    def async_compute(self, value: TaskPoolThreadAssignmentPolicy) -> None: ...
    @property
    def compute(self) -> TaskPoolThreadAssignmentPolicy: ...
    @compute.setter
    def compute(self, value: TaskPoolThreadAssignmentPolicy) -> None: ...

class TaskPoolPlugin(Plugin):
    """Install Bevy's process-global pools with the supplied options.

    The first built plugin initializes the pools; later apps cannot resize them.
    The options and nested assignment-policy properties are live configuration
    objects, so field edits made before adding the plugin are preserved.
    """

    def __init__(self, *, task_pool_options: TaskPoolOptions | None = None) -> None: ...
    @property
    def task_pool_options(self) -> TaskPoolOptions: ...
    @task_pool_options.setter
    def task_pool_options(self, value: TaskPoolOptions) -> None: ...
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
    def trigger_reload_if_needed(self) -> None:
        """Trigger reload if not already pending, using current default mode."""
    def get_default_mode(self) -> str:
        """Get the current default reload mode ('Full' or 'Partial')."""
    def is_partial_reload(self) -> bool:
        """Check if the next reload will be in partial mode."""
    def generation(self) -> int:
        """Get the generation shared with HotReloadControl."""

class HotReloadControl(Resource):
    """Control hot reload behavior from within systems.

    This resource allows systems to request reloads dynamically, such as when
    the user presses F5 for a full reload.

    Example:
        ```python
        def handle_f5(input: Res[ButtonInput[KeyCode]], world: World) -> None:
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
    """A Bevy application: its worlds, plugins, schedules, and runner.

    A schedule whose graph cannot be built raises `RuntimeError` from
    `update()`, `run()`, or `run_schedule()`. Discard the App afterward:
    bevy takes a schedule out of the world to run it and puts it back only
    on success, so that schedule is gone and later calls skip it or panic.
    """

    def __init__(self) -> None: ...
    def add_systems(self, schedule: Stage | OnEnterSchedule | OnExitSchedule | OnTransitionSchedule, *systems: SystemFns) -> App: ...
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
    def is_plugin_added(self, plugin_type: type[Plugin]) -> bool: ...
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
                app_exit_writer.write(AppExit.Success())

            def verify_exit(app: Res[App]) -> None:
                exit_status = app.should_exit()
                if isinstance(exit_status, AppExit.Success):
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

    def configure_sets(
        self,
        schedule: Stage,
        *sets: SystemSetChainItem | ChainedSystemSets,
    ) -> App:
        """Configure set hierarchy, ordering, and shared run conditions."""

    def cleanup(self) -> None:
        """Clean up app resources."""
    def clear_scene(self) -> None:
        """Clear the scene by despawning all entities and clearing custom resources.

        This is similar to hot reload's Full mode but without reloading systems.

        Preserves:
        - Built-in Bevy resources (Time, AssetServer, etc.)
        - RenderDevice and render infrastructure
        - Plugin state

        Clears:
        - All entities
        - Custom Python resources
        """

    def _set_hot_reload_loader(
        self, loader: Callable[[], Callable[[App], App]]
    ) -> App: ...
    def _set_scene_module(self, module_name: str) -> App: ...


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

    class Loop(RunMode):
        """Run the schedule repeatedly, optionally waiting between frames."""
        __match_args__: ClassVar[tuple[Literal["wait"]]]
        wait: int | None
        def __init__(self, *, wait: int | None = None) -> None: ...

    class Once(RunMode):
        """Run the schedule exactly once and then exit."""
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

class ScheduleRunnerPlugin(Plugin):
    """Configures an App to run its schedule according to a given RunMode.

    This plugin is essential for headless applications and testing. It controls
    the app's execution loop, determining whether it runs once or continuously.

    **Plugin Groups:**
    - Included in MinimalPlugins
    - NOT included in DefaultPlugins (which uses WinitPlugin's event loop instead)

    **Testing Usage:**
    Use `ScheduleRunnerPlugin.run_once()` when testing the full schedule lifecycle.
    For simple setup/query tests, use `App()._run_systems_once(...)`; for multiple
    frames, call `app.update()` repeatedly.

    Args:
        run_mode: Controls execution behavior (default: RunMode.Loop())

    Examples:
        ```python
        # Testing - run once and exit
        app = App()
        app.add_plugins(ScheduleRunnerPlugin.run_once())
        app.add_systems(Startup, setup)
        app.add_systems(Update, game_logic)
        app.run()  # Runs full schedule once

        # Headless server - run continuously
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(run_mode=RunMode.Loop()))
        app.add_systems(Update, server_tick)
        app.run()  # Runs forever

        # Frame-limited loop
        app = App()
        app.add_plugins(ScheduleRunnerPlugin(run_mode=RunMode.Loop(wait=16)))
        app.run()  # Runs at ~60 FPS
        ```

    Notes:
        - For graphical applications, use DefaultPlugins instead (includes WinitPlugin)
        - Use run_once() for a single-frame schedule lifecycle
        - Startup systems run before the first Update when using RunMode.Once()
    """

    def __init__(self, *, run_mode: RunMode = ...) -> None:
        """Create a ScheduleRunnerPlugin with the specified run mode.

        Args:
            run_mode: Execution mode (default: RunMode.Loop())
        """

    def build(self, app: App) -> None:
        """Build and apply the plugin to the app."""

    @staticmethod
    def run_once() -> ScheduleRunnerPlugin:
        """Create a plugin configured to run the schedule once.

        Equivalent to `ScheduleRunnerPlugin(run_mode=RunMode.Once())`.
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

        Equivalent to `ScheduleRunnerPlugin(run_mode=RunMode.Loop(wait=...))`.

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
    """Exit status emitted by a Bevy application."""

    class Success(AppExit):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Error(AppExit):
        __match_args__: ClassVar[tuple[Literal["code"]]]
        code: int
        def __init__(self, code: int) -> None: ...

    @staticmethod
    def error() -> AppExit.Error:
        """Create an AppExit::Error with exit code 1."""
    @staticmethod
    def from_code(code: int) -> AppExit:
        """Exit with the given code; 0 becomes Success."""
    def is_success(self) -> bool:
        """True if this is AppExit::Success."""
    def is_error(self) -> bool:
        """True if this is AppExit::Error."""
