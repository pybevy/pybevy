"""Shows how to create systems that run every fixed timestep, rather than every tick.

This example demonstrates the difference between Update (runs every frame) and
FixedUpdate (runs at fixed intervals). The FixedUpdate system will run twice
per second regardless of the actual frame rate.
"""

from dataclasses import dataclass

from pybevy.prelude import *
from pybevy.time import TimeFixed


@dataclass
class FloatWrapper:
    """Wrapper for float to use with Local.

    Local[float] doesn't work because float is a primitive.
    Local proxies attribute access to the wrapped object.
    """
    value: float = 0.0


def frame_update(last_time: Local[FloatWrapper], time: Res[Time]) -> None:
    """Run once per frame (matches your screen's refresh rate).

    Default Time is Time (virtual time) in Update schedule.
    """
    # Initialize last_time on first run
    if last_time.value.value == 0.0:
        last_time.value.value = time.elapsed_secs()
        return

    delta = time.elapsed_secs() - last_time.value.value
    print(f"time since last frame_update: {delta:.6f}")
    last_time.value.value = time.elapsed_secs()


def fixed_update(
    last_time: Local[FloatWrapper],
    time: Res[Time],
    fixed_time: Res[TimeFixed]
) -> None:
    """Run at fixed intervals (twice per second in this example).

    Default Time in FixedUpdate schedule provides fixed timestep information.
    """
    # Initialize last_time on first run
    if last_time.value.value == 0.0:
        last_time.value.value = time.elapsed_secs()
        print("Starting fixed timestep updates (every 0.5 seconds)\n")
        return

    delta = time.elapsed_secs() - last_time.value.value
    print(f"time since last fixed_update: {delta:.6f}")
    print(f"fixed timestep: {time.delta_secs():.6f}")

    # If we want to see the overstep, we need to access TimeFixed specifically
    print(f"time accrued toward next fixed_update: {fixed_time.overstep().total_seconds():.6f}\n")

    last_time.value.value = time.elapsed_secs()


@entrypoint
def main(app: App) -> App:
    """Configure the app with fixed and variable timestep systems."""
    return (
        app.add_plugins(DefaultPlugins)
        # This system will run once every update (matches screen refresh rate)
        .add_systems(Update, frame_update)
        # This system runs at fixed intervals
        .add_systems(FixedUpdate, fixed_update)
        # Configure fixed timestep schedule to run twice per second
        .insert_resource(TimeFixed.from_seconds(0.5))
    )


if __name__ == "__main__":
    main().run()
