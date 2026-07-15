from datetime import timedelta

from pybevy.app import App, Plugin
from pybevy.ecs import Resource

# Type alias for duration parameters - can be timedelta, float seconds, or int seconds
Duration = timedelta | float | int

class TimePlugin(Plugin):
    """Adds time functionality to Bevy applications.

    This plugin initializes and manages all time-related resources in the ECS world.
    It automatically inserts Time, Time<Real>, Time<Virtual>, and Time<Fixed> resources,
    and configures the time update systems that run each frame.

    **Resources Inserted:**
    - `Time` - Default time context (Virtual in Update, Fixed in FixedUpdate)
    - `Time<Real>` - Real wall-clock time (unaffected by pause/speed)
    - `Time<Virtual>` - Virtual game time (can be paused or time-scaled)
    - `Time<Fixed>` - Fixed timestep time for physics/deterministic logic

    **Plugin Groups:**
    - Included in DefaultPlugins
    - Included in MinimalPlugins

    **System Sets:**
    - TimeSystems runs in First schedule to update all time resources

    Example:
        ```python
        from pybevy.app import App
        from pybevy.time import TimePlugin

        # TimePlugin is included in DefaultPlugins
        app = App().add_plugins(DefaultPlugins)

        # Or add explicitly if using custom plugin setup
        app = App().add_plugins(TimePlugin())
        ```

    Notes:
        - The time system must run before any systems that use Time resources
        - Time<Virtual> can be paused or speed-adjusted via ResMut[TimeVirtual]
        - Time<Fixed> timestep is configurable via ResMut[TimeFixed]
    """

    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class TimeUpdateStrategy(Resource):
    """Controls how Bevy advances its real and virtual clocks each frame."""

    def __init__(self) -> None:
        """Create Bevy's default automatic update strategy."""

    @staticmethod
    def Automatic() -> TimeUpdateStrategy:
        """Advance from the render-world or system clock."""

    @staticmethod
    def ManualDuration(duration: timedelta) -> TimeUpdateStrategy:
        """Advance by ``duration`` after Bevy's zero-delta first update."""

    @staticmethod
    def FixedTimesteps(steps: int) -> TimeUpdateStrategy:
        """Advance by ``steps`` fixed timesteps on every app update."""

class Time(Resource):
    """A generic clock resource that tracks elapsed time and delta time.

    This is the main time resource used by Bevy systems. The `Time` resource
    automatically switches contexts:
    - In Update schedule: refers to Time<Virtual> (game time, pauseable)
    - In FixedUpdate schedule: refers to Time<Fixed> (fixed timestep)

    **Key Concepts:**
    - **Delta time**: Time elapsed since the previous update (frame time)
    - **Elapsed time**: Total time since clock creation
    - **Wrap period**: Prevents f32 precision loss by wrapping elapsed time

    **Time Contexts:**
    - Use `Res[Time]` for context-aware time (recommended for most systems)
    - Use `Res[Time<Real>]` for real wall-clock time (UI, profiling)
    - Use `Res[Time<Virtual>]` for explicit virtual game time access
    - Use `Res[Time<Fixed>]` for explicit fixed timestep access

    The time values are available as exact Duration with nanosecond precision,
    or as f32/f64 seconds for convenience. The clock does not support moving
    backwards in time.

    Example:
        ```python
        from pybevy.ecs import Res
        from pybevy.time import Time

        def update_animation(time: Res[Time]) -> None:
            # Delta time - time since last frame
            dt = time.delta_secs()  # e.g., 0.016 for 60 FPS

            # Elapsed time - total time since start
            elapsed = time.elapsed_secs()

            # Wrapped elapsed - prevents f32 precision loss
            wrapped = time.elapsed_secs_wrapped()  # wraps every hour by default
        ```

    Notes:
        - Frame delta time varies with monitor refresh rate and system load
        - Use elapsed_wrapped() for shader time uniforms to avoid precision loss
        - Time<Virtual> can be paused or sped up via ResMut[TimeVirtual]

    See Also:
        - TimeVirtual: Pauseable game time with speed control
        - TimeFixed: Fixed timestep time for deterministic logic
    """

    def __init__(self) -> None: ...

    def advance_by(self, delta: timedelta) -> None:
        """Manually advance the clock by a specific duration.

        Primarily used for testing and manual time control. In normal
        applications, time is advanced automatically by TimePlugin.

        Args:
            delta: Duration to advance the clock by

        Example:
            ```python
            time = Time()
            time.advance_by(timedelta(milliseconds=16))  # Simulate one frame at 60 FPS
            assert time.delta_secs() == 0.016
            ```
        """

    def advance_to(self, elapsed: timedelta) -> None:
        """Manually set the clock to a specific elapsed time.

        Sets the elapsed time directly and updates delta accordingly.
        Primarily used for testing.

        Args:
            elapsed: The target elapsed time

        Example:
            ```python
            time = Time()
            time.advance_to(timedelta(seconds=10))
            assert time.elapsed_secs() == 10.0
            ```
        """

    def set_wrap_period(self, wrap_period: timedelta) -> None:
        """Set the wrap period for elapsed time.

        The wrap period prevents f32 precision loss by wrapping elapsed time
        back to zero after the specified duration. Default is 1 hour.

        Args:
            wrap_period: Duration after which elapsed_wrapped() wraps to zero

        Example:
            ```python
            time.set_wrap_period(timedelta(minutes=5))  # Wrap every 5 minutes
            ```

        Notes:
            - Only affects elapsed_wrapped() methods, not elapsed()
            - Useful for shader time uniforms to maintain precision
        """

    def wrap_period(self) -> timedelta:
        """Get the current wrap period.

        Returns:
            The duration after which elapsed_wrapped() wraps to zero
        """

    def delta(self) -> timedelta:
        """Get the time elapsed since the previous update.

        Returns frame delta time as a Duration with nanosecond precision.

        Returns:
            Time elapsed since last frame (typically 16ms for 60 FPS)

        Example:
            ```python
            def move_player(time: Res[Time]) -> None:
                dt = time.delta()
                # Use for frame-rate independent movement
                position += velocity * dt.total_seconds()
            ```
        """

    def delta_secs(self) -> float:
        """Get the delta time in seconds as f32.

        Returns:
            Delta time in seconds (e.g., 0.016 for 60 FPS)
        """

    def delta_secs_f64(self) -> float:
        """Get the delta time in seconds as f64 (higher precision).

        Returns:
            Delta time in seconds with f64 precision
        """

    def elapsed(self) -> timedelta:
        """Get the total time the clock has advanced since creation.

        Returns:
            Total elapsed time as Duration

        Example:
            ```python
            def check_timeout(time: Res[Time]) -> None:
                if time.elapsed() > timedelta(minutes=5):
                    print("5 minutes have passed!")
            ```
        """

    def elapsed_secs(self) -> float:
        """Get the total elapsed time in seconds as f32.

        Returns:
            Total elapsed time in seconds

        Notes:
            - This value grows continuously and will lose precision over time
            - For long-running apps, prefer elapsed_secs_wrapped()
        """

    def elapsed_secs_f64(self) -> float:
        """Get the total elapsed time in seconds as f64 (higher precision).

        Returns:
            Total elapsed time in seconds with f64 precision
        """

    def elapsed_wrapped(self) -> timedelta:
        """Get elapsed time wrapped to the wrap period.

        Prevents precision loss by wrapping after the configured period.
        Default wrap period is 1 hour.

        Returns:
            Elapsed time wrapped to [0, wrap_period)

        Example:
            ```python
            # For shader time uniforms
            shader_time = time.elapsed_secs_wrapped()
            ```
        """

    def elapsed_secs_wrapped(self) -> float:
        """Get wrapped elapsed time in seconds as f32.

        Returns:
            Wrapped elapsed time in seconds
        """

    def elapsed_secs_wrapped_f64(self) -> float:
        """Get wrapped elapsed time in seconds as f64.

        Returns:
            Wrapped elapsed time in seconds with f64 precision
        """

class TimeVirtual(Resource):
    """Virtual game time resource with pause and speed control.

    This is the specialized Time<Virtual> resource that allows pausing
    and controlling the speed of game time. Use ResMut[TimeVirtual] to
    access pause/unpause and speed control methods.

    The virtual clock can be paused, sped up, or slowed down independently
    of real time. It also clamps maximum delta to prevent large jumps.
    """

    def __init__(self) -> None: ...

    # Pause control
    def pause(self) -> None:
        """Pause the virtual clock.

        When paused, delta() returns zero and elapsed() does not grow.
        """

    def unpause(self) -> None:
        """Resume the virtual clock if paused."""

    def is_paused(self) -> bool:
        """Returns true if the virtual clock is currently paused."""

    def was_paused(self) -> bool:
        """Returns true if the virtual clock was paused at the start of this update."""

    # Speed control
    def set_relative_speed(self, ratio: float) -> None:
        """Set the speed multiplier relative to real time.

        Args:
            ratio: Speed multiplier (2.0 = twice as fast, 0.5 = half speed)

        Example:
            # Slow motion effect
            time_virtual.set_relative_speed(0.5)

            # Fast forward
            time_virtual.set_relative_speed(2.0)
        """

    def set_relative_speed_f64(self, ratio: float) -> None:
        """Set the speed multiplier (f64 precision)."""

    def relative_speed(self) -> float:
        """Get the current speed multiplier."""

    def relative_speed_f64(self) -> float:
        """Get the current speed multiplier (f64 precision)."""

    def effective_speed(self) -> float:
        """Get the effective speed for this update.

        Returns 0.0 if paused, otherwise returns relative_speed.
        """

    def effective_speed_f64(self) -> float:
        """Get the effective speed for this update (f64 precision)."""

    # Delta clamping
    def max_delta(self) -> timedelta:
        """Get the maximum delta time allowed in a single update."""

    def set_max_delta(self, max_delta: timedelta) -> None:
        """Set the maximum delta time allowed in a single update.

        This prevents large time jumps (e.g., after tab switching or suspend).
        Default is 250ms.
        """

    # Base Time methods
    def delta(self) -> timedelta: ...
    def delta_secs(self) -> float: ...
    def delta_secs_f64(self) -> float: ...
    def elapsed(self) -> timedelta: ...
    def elapsed_secs(self) -> float: ...
    def elapsed_secs_f64(self) -> float: ...
    def advance_by(self, delta: timedelta) -> None: ...
    def advance_to(self, elapsed: timedelta) -> None: ...
    def wrap_period(self) -> timedelta: ...
    def set_wrap_period(self, wrap_period: timedelta) -> None: ...
    def elapsed_wrapped(self) -> timedelta: ...
    def elapsed_secs_wrapped(self) -> float: ...
    def elapsed_secs_wrapped_f64(self) -> float: ...

class TimeFixed(Resource):
    """Fixed timestep time resource.

    This clock advances in fixed-size increments, which is useful for
    physics and other logic that should have consistent behavior regardless
    of framerate. Use with the FixedUpdate stage.

    The default timestep is 64 Hz (15.625ms). This was chosen to avoid
    pathological interactions with typical 60Hz monitor refresh rates.
    """

    def __init__(self) -> None: ...

    @staticmethod
    def from_duration(timestep: timedelta) -> TimeFixed:
        """Create a TimeFixed with a specific timestep as Duration.

        Args:
            timestep: Timestep duration as timedelta

        Example:
            app.insert_resource(TimeFixed.from_duration(timedelta(milliseconds=16)))
        """

    @staticmethod
    def from_hz(hz: float) -> TimeFixed:
        """Create a TimeFixed with a specific timestep frequency.

        Args:
            hz: Frequency in Hertz (e.g., 60.0 for 60 updates per second)

        Example:
            app.insert_resource(TimeFixed.from_hz(60.0))
        """

    @staticmethod
    def from_seconds(seconds: float) -> TimeFixed:
        """Create a TimeFixed with a specific timestep duration.

        Args:
            seconds: Timestep duration in seconds (e.g., 0.016666 for ~60Hz)

        Example:
            app.insert_resource(TimeFixed.from_seconds(1.0 / 60.0))
        """

    def timestep(self) -> timedelta:
        """Get the fixed timestep duration."""

    def set_timestep(self, timestep: timedelta) -> None:
        """Set the fixed timestep duration."""

    def set_timestep_seconds(self, seconds: float) -> None:
        """Set the fixed timestep in seconds."""

    def set_timestep_hz(self, hz: float) -> None:
        """Set the fixed timestep in Hz (frequency)."""

    def overstep(self) -> timedelta:
        """Get the accumulated time beyond complete timesteps."""

    def discard_overstep(self, discard: timedelta) -> None:
        """Discard a part of the overstep amount."""

    def overstep_fraction(self) -> float:
        """Get the overstep as an f32 fraction of the timestep."""

    def overstep_fraction_f64(self) -> float:
        """Get the overstep as an f64 fraction of the timestep."""

    def delta(self) -> timedelta:
        """Get the delta time for this fixed step."""

    def delta_secs(self) -> float:
        """Get the delta time for this fixed step in seconds."""

    def delta_secs_f64(self) -> float:
        """Get the delta time for this fixed step in seconds (f64 precision)."""

    def elapsed(self) -> timedelta:
        """Get the total elapsed time."""

    def elapsed_secs(self) -> float:
        """Get the total elapsed time in seconds."""

    def elapsed_secs_f64(self) -> float:
        """Get the total elapsed time in seconds (f64 precision)."""

    def advance_to(self, elapsed: timedelta) -> None:
        """Advance the clock to a specific elapsed time."""

    def advance_by(self, delta: timedelta) -> None:
        """Advance the clock by a specific duration."""

    def wrap_period(self) -> timedelta:
        """Get the current wrap period."""

    def set_wrap_period(self, wrap_period: timedelta) -> None:
        """Set the wrap period for elapsed time."""

    def elapsed_wrapped(self) -> timedelta:
        """Get elapsed time wrapped to the wrap period."""

    def elapsed_secs_wrapped(self) -> float:
        """Get wrapped elapsed time in seconds as f32."""

    def elapsed_secs_wrapped_f64(self) -> float:
        """Get wrapped elapsed time in seconds as f64."""

class TimerMode:
    """Specifies Timer behavior when the duration is reached.

    **Modes:**
    - **ONCE**: Timer runs once and stays finished until manually reset
    - **REPEATING**: Timer automatically resets and continues when finished

    Example:
        ```python
        from pybevy.time import Timer, TimerMode

        # One-shot timer for cooldowns
        cooldown = Timer(timedelta(seconds=5), TimerMode.ONCE)

        # Repeating timer for periodic events
        spawn_timer = Timer(timedelta(seconds=2), TimerMode.REPEATING)
        ```

    See Also:
        - Timer: The timer class that uses this mode enum
    """

    ONCE: TimerMode
    """Run once and stop. Timer stays finished until reset() is called."""

    REPEATING: TimerMode
    """Reset automatically when finished and continue counting."""

    def __eq__(self, other: object) -> bool: ...
    def __str__(self) -> str:
        """Get string representation of the timer mode.

        Returns:
            "once" or "repeating"
        """

class Timer:
    """A timer that tracks elapsed time and can trigger when finished.

    Timers are useful for cooldowns, spawn rates, animations, and any time-based
    game logic. They must be manually ticked each frame to advance.

    **Timer Modes:**
    - **ONCE**: Runs once and stays finished until reset()
    - **REPEATING**: Automatically resets and continues when finished

    **Key Features:**
    - Pause/unpause support
    - Query progress via fraction() and remaining()
    - Repeating timers can finish multiple times per tick
    - Supports timedelta, float seconds, or int seconds

    Args:
        duration: Timer duration - can be a timedelta, float (seconds), or int (seconds)
        mode: Timer mode - TimerMode.ONCE or TimerMode.REPEATING

    Example:
        ```python
        from datetime import timedelta
        from pybevy.time import Timer, TimerMode
        from pybevy.ecs import Res

        # Create a 5-second cooldown timer
        cooldown = Timer(timedelta(seconds=5), TimerMode.ONCE)

        def update_timer(time: Res[Time]) -> None:
            cooldown.tick(time.delta())

            if cooldown.just_finished():
                print("Cooldown complete!")

            if cooldown.is_finished():
                # Can perform action
                perform_action()
                cooldown.reset()  # Start cooldown again
        ```

    Notes:
        - Paused timers do not advance when ticked
        - Repeating timers can wrap multiple times in one tick
        - Use just_finished() to detect completion, not is_finished()

    See Also:
        - TimerMode: Enum for timer behavior
        - Time: For getting delta time to tick with
    """

    def __init__(
        self, duration: Duration | None = None, mode: TimerMode = ...
    ) -> None:
        """Create a new timer.

        If called with no arguments, creates a timer with duration 0.0 and
        TimerMode.ONCE (equivalent to Bevy's Timer::default()).

        Args:
            duration: Timer duration - can be timedelta, float (seconds), or int (seconds).
                     Defaults to 0.0 if not provided.
            mode: Timer mode - TimerMode.ONCE or TimerMode.REPEATING. Defaults to ONCE.
        """

    @staticmethod
    def from_seconds(duration: float, mode: TimerMode) -> Timer:
        """Create a new timer with a given duration in seconds.

        Convenience method equivalent to `Timer(duration, mode)`.

        Args:
            duration: Timer duration in seconds
            mode: Timer mode - TimerMode.ONCE or TimerMode.REPEATING

        Returns:
            A new Timer instance

        Example:
            ```python
            # More concise than Timer(timedelta(seconds=5), TimerMode.ONCE)
            cooldown = Timer.from_seconds(5.0, TimerMode.ONCE)

            # Matches Bevy's Rust API
            spawn_timer = Timer.from_seconds(2.0, TimerMode.REPEATING)
            ```
        """

    def tick(self, delta: Duration) -> None:
        """Advance the timer by delta time.

        For non-repeating timers, elapsed time is clamped at duration.
        For repeating timers, elapsed time wraps around when duration is reached.
        Paused timers are not affected.

        Args:
            delta: Time delta - can be a timedelta, float (seconds), or int (seconds)

        Example:
            ```python
            def update_system(time: Res[Time]) -> None:
                timer.tick(time.delta())  # Advance by frame delta
            ```

        Notes:
            - Repeating timers may finish multiple times if delta > duration
            - Check times_finished_this_tick() for multiple completions
        """

    def finished(self) -> bool:
        """Check if the timer has reached its duration.

        For repeating timers, returns True only on ticks when duration was reached.
        For non-repeating timers, returns True once finished until reset.

        Returns:
            True if timer is finished

        Example:
            ```python
            if timer.finished():
                # Timer has completed
                timer.reset()
            ```

        See Also:
            - just_finished(): Detects the exact tick when timer finished
        """

    def is_finished(self) -> bool:
        """Alias for finished()."""

    def just_finished(self) -> bool:
        """Check if the timer finished on this exact tick.

        Unlike is_finished(), this returns True only on the tick when
        the timer reached its duration, not on subsequent ticks.

        Returns:
            True if timer finished this tick

        Example:
            ```python
            if timer.just_finished():
                # Trigger event exactly once when timer completes
                spawn_enemy()
            ```

        Notes:
            - Preferred over is_finished() for one-time events
            - For repeating timers, can be True multiple times per tick
        """

    def elapsed(self) -> timedelta:
        """Get the elapsed time as a Duration.

        Guaranteed to be between 0 and duration. For non-repeating timers,
        equals duration when finished.

        Returns:
            Elapsed time as Duration
        """

    def duration(self) -> timedelta:
        """Get the timer duration.

        Returns:
            Timer duration as Duration
        """

    def set_duration(self, duration: Duration) -> None:
        """Set the timer duration.

        Args:
            duration: New duration - can be a timedelta, float (seconds), or int (seconds)

        Example:
            ```python
            # Adjust spawn rate dynamically
            if difficulty == "hard":
                spawn_timer.set_duration(1.0)  # Spawn every 1 second
            else:
                spawn_timer.set_duration(3.0)  # Spawn every 3 seconds
            ```
        """

    def reset(self) -> None:
        """Reset the timer to zero elapsed time.

        Clears the finished state and restarts counting from zero.

        Example:
            ```python
            if action_performed:
                cooldown_timer.reset()  # Start cooldown
            ```
        """

    def pause(self) -> None:
        """Pause the timer.

        Paused timers do not advance when ticked.

        Example:
            ```python
            if game_paused:
                timer.pause()
            ```
        """

    def unpause(self) -> None:
        """Resume a paused timer.

        Example:
            ```python
            if game_resumed:
                timer.unpause()
            ```
        """

    def paused(self) -> bool:
        """Check if the timer is paused.

        Returns:
            True if timer is paused
        """

    def is_paused(self) -> bool:
        """Alias for paused()."""

    def fraction(self) -> float:
        """Get the progress as a fraction between 0.0 and 1.0.

        Returns:
            Elapsed time / duration (0.0 = just started, 1.0 = finished)

        Example:
            ```python
            # Update a progress bar
            progress_bar.set_value(timer.fraction())
            ```
        """

    def fraction_remaining(self) -> float:
        """Get the remaining progress as a fraction between 0.0 and 1.0.

        Returns:
            Remaining time / duration (1.0 = just started, 0.0 = finished)

        Example:
            ```python
            # Show cooldown overlay opacity
            overlay.alpha = timer.fraction_remaining()
            ```
        """

    def remaining(self) -> timedelta:
        """Get the remaining time as a Duration.

        Returns:
            Duration - elapsed time
        """

    def times_finished_this_tick(self) -> int:
        """Get the number of times the timer finished this tick.

        For repeating timers ticked with a large delta, this can be > 1.
        For non-repeating timers, this is always 0 or 1.

        Returns:
            Number of times timer finished in this tick

        Example:
            ```python
            # Handle multiple spawns if frame took long
            count = spawn_timer.times_finished_this_tick()
            for _ in range(count):
                spawn_enemy()
            ```

        Notes:
            - Only meaningful for repeating timers with large deltas
            - Same as 1 if just_finished() is True for ONCE timers
        """

    def elapsed_secs(self) -> float:
        """Get the elapsed time in seconds as f32.

        Returns:
            Elapsed time in seconds
        """

    def elapsed_secs_f64(self) -> float:
        """Get the elapsed time in seconds as f64 (higher precision).

        Returns:
            Elapsed time in seconds with f64 precision
        """

    def remaining_secs(self) -> float:
        """Get the remaining time in seconds.

        Returns:
            Duration - elapsed time in seconds
        """

    def set_elapsed(self, time: Duration) -> None:
        """Set the elapsed time of the timer.

        Args:
            time: The elapsed time to set

        Notes:
            - Setting elapsed time does not affect finished state
            - Call tick() after set_elapsed() if you want to update finished state
        """

    def mode(self) -> TimerMode:
        """Get the timer mode.

        Returns:
            The current timer mode (ONCE or REPEATING)
        """

    def set_mode(self, mode: TimerMode) -> None:
        """Set the timer mode.

        Args:
            mode: The new timer mode (TimerMode.ONCE or TimerMode.REPEATING)

        Notes:
            - If switching from ONCE to REPEATING while finished, the timer resets
        """

    def finish(self) -> None:
        """Immediately finish the timer.

        Advances the timer to its duration, triggering the finished state.
        """

    def almost_finish(self) -> None:
        """Advance the timer to 1 nanosecond before finishing.

        Useful for testing: the next tick() with any positive duration will finish the timer.
        """

    def __eq__(self, other: object) -> bool: ...


class Stopwatch:
    """A stopwatch that tracks elapsed time when started.

    Unlike Timer, Stopwatch doesn't have a duration - it just counts time.
    Like Timer, it must be manually ticked each frame to advance.

    **Key Features:**
    - Pause/unpause support
    - Query elapsed time in various formats
    - Reset to zero

    Example:
        ```python
        from pybevy.time import Stopwatch
        from pybevy.ecs import Res
        from pybevy.time import Time

        # Create a new stopwatch
        stopwatch = Stopwatch()

        def update_stopwatch(time: Res[Time]) -> None:
            stopwatch.tick(time.delta())
            print(f"Elapsed: {stopwatch.elapsed_secs():.2f}s")
        ```
    """

    def __init__(self) -> None: ...

    def tick(self, delta: Duration) -> None:
        """Advance the stopwatch by delta time.

        Paused stopwatches are not affected by ticking.

        Args:
            delta: Time delta - can be a timedelta, float (seconds), or int (seconds)
        """

    def elapsed(self) -> timedelta:
        """Get the elapsed time as a Duration.

        Returns:
            Total elapsed time
        """

    def elapsed_secs(self) -> float:
        """Get the elapsed time in seconds as f32.

        Returns:
            Elapsed time in seconds
        """

    def elapsed_secs_f64(self) -> float:
        """Get the elapsed time in seconds as f64 (higher precision).

        Returns:
            Elapsed time in seconds with f64 precision
        """

    def set_elapsed(self, time: Duration) -> None:
        """Set the elapsed time of the stopwatch.

        Args:
            time: The elapsed time to set
        """

    def pause(self) -> None:
        """Pause the stopwatch.

        Paused stopwatches do not advance when ticked.
        """

    def unpause(self) -> None:
        """Resume a paused stopwatch."""

    def is_paused(self) -> bool:
        """Check if the stopwatch is paused.

        Returns:
            True if the stopwatch is paused
        """

    def reset(self) -> None:
        """Reset the stopwatch to zero elapsed time.

        The reset doesn't affect the paused state.
        """

    def __eq__(self, other: object) -> bool: ...


class TimeReal(Resource):
    """Real wall-clock time resource.

    This is the specialized Time<Real> resource that tracks actual wall-clock time.
    It is not affected by pause, speed adjustments, or other virtual time modifications.

    Use this for:
    - UI animations that shouldn't pause with the game
    - Profiling and performance measurement
    - Real-time networking timestamps

    The clock does not count time from startup to first update into elapsed,
    but instead starts counting from the first update call.
    """

    def __init__(self) -> None: ...

    def delta(self) -> timedelta:
        """Get the time elapsed since the previous update.

        Returns:
            Time elapsed since last frame
        """

    def delta_secs(self) -> float:
        """Get the delta time in seconds as f32.

        Returns:
            Delta time in seconds
        """

    def delta_secs_f64(self) -> float:
        """Get the delta time in seconds as f64 (higher precision).

        Returns:
            Delta time in seconds with f64 precision
        """

    def elapsed(self) -> timedelta:
        """Get the total time the clock has advanced since first update.

        Returns:
            Total elapsed time as Duration
        """

    def elapsed_secs(self) -> float:
        """Get the total elapsed time in seconds as f32.

        Returns:
            Total elapsed time in seconds
        """

    def elapsed_secs_f64(self) -> float:
        """Get the total elapsed time in seconds as f64 (higher precision).

        Returns:
            Total elapsed time in seconds with f64 precision
        """

    def advance_to(self, elapsed: timedelta) -> None:
        """Advance the clock to a specific elapsed time."""

    def advance_by(self, delta: timedelta) -> None:
        """Advance the clock by a specific duration."""

    def wrap_period(self) -> timedelta:
        """Get the current wrap period."""

    def set_wrap_period(self, wrap_period: timedelta) -> None:
        """Set the wrap period for elapsed time."""

    def elapsed_wrapped(self) -> timedelta:
        """Get elapsed time wrapped to the wrap period."""

    def elapsed_secs_wrapped(self) -> float:
        """Get wrapped elapsed time in seconds as f32."""

    def elapsed_secs_wrapped_f64(self) -> float:
        """Get wrapped elapsed time in seconds as f64."""


class Fixed:
    """Marker type for fixed timestep time.

    Used as a type parameter for Time[Fixed] resources in type annotations.
    This represents time that advances in fixed increments, useful for physics
    and deterministic game logic.

    Example:
        ```python
        from pybevy.ecs import Res
        from pybevy.time import Time, Fixed

        # In Bevy Rust: Res<Time<Fixed>>
        def fixed_update(time: Res[Time]) -> None:
            # This system runs in FixedUpdate schedule
            pass
        ```
    """

    def __init__(self) -> None: ...
    def __repr__(self) -> str: ...


class Real:
    """Marker type for real (wall-clock) time.

    Used as a type parameter for Time[Real] resources in type annotations.
    This represents actual wall-clock time that is not affected by pause
    or speed adjustments.

    Example:
        ```python
        from pybevy.ecs import Res
        from pybevy.time import TimeReal

        # Real time is unaffected by game pause
        def ui_animation(time_real: Res[TimeReal]) -> None:
            # UI keeps animating even when game is paused
            pass
        ```
    """

    def __init__(self) -> None: ...
    def __repr__(self) -> str: ...


class Virtual:
    """Marker type for virtual (game) time.

    Used as a type parameter for Time[Virtual] resources in type annotations.
    This represents virtual game time that can be paused, sped up, or slowed down.

    Example:
        ```python
        from pybevy.ecs import ResMut
        from pybevy.time import TimeVirtual

        # Control game time speed
        def toggle_slow_motion(time: ResMut[TimeVirtual]) -> None:
            if time.relative_speed() == 1.0:
                time.set_relative_speed(0.5)  # Slow motion
            else:
                time.set_relative_speed(1.0)  # Normal speed
        ```
    """

    def __init__(self) -> None: ...
    def __repr__(self) -> str: ...
