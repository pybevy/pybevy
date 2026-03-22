"""Example demonstrating View API performance with 20 million entities.

This example uses MinimalPlugins for a headless app and spawns 20 million entities
using batch spawning, then modifies them using the View API which is 30-50x faster
than Query iteration.

Uses Transform component to demonstrate View API's ability to work with
Bevy's built-in components and modify their fields in batch.
"""

import numpy as np

from pybevy.ecs import World
from pybevy.prelude import *


@component
class Marker(Component):
    """Marker component for all spawned entities."""



class FPSCounter:
    """Local state for tracking FPS calculations."""

    def __init__(self) -> None:
        self.last_time: float = 0.0
        self.frame_count: int = 0


COUNT = 20_000_000


def spawn_entities(world: World) -> None:
    """Spawn {COUNT} entities with Transform using batch spawning."""
    print(f"Spawning {COUNT:,} entities...")
    grid_size = int(COUNT ** (1.0 / 3.0))

    i = np.arange(COUNT, dtype=np.float32)
    x = (i % grid_size) * 0.5
    y = ((i // grid_size) % grid_size) * 0.5
    z = (i // (grid_size * grid_size)) * 0.5

    positions = np.stack([x, y, z], axis=1)
    batch = Transform.from_numpy(translation=positions)
    entities = world.commands().spawn_batch(batch, Marker())
    print(f"Spawned {len(entities):,} entities.")


def update_positions(
    time: Res[Time],
    view: View[Mut[Transform], With[Marker]],
) -> None:
    """Update all Transform positions using View API for maximum performance."""
    t = time.elapsed_secs()

    # Get column proxy for batch operations
    transform = view.column_mut(Transform)

    # Create a wave pattern that moves through the entities
    # Update all Transform.translation.y fields in batch - 30-50x faster than Query iteration!
    transform.translation.y = (
        transform.translation.y + (transform.translation.x * 0.01 + t).sin() * 0.1
    )


def print_stats(
    time: Res[Time],
    fps_counter: Local[FPSCounter],
) -> None:
    """Print statistics about the entities."""
    current_time = time.elapsed_secs()
    fps_counter.frame_count += 1

    # Print stats once per second
    time_since_last = current_time - fps_counter.last_time
    if time_since_last >= 1.0:
        # Calculate FPS based on frames since last print
        fps = fps_counter.frame_count / time_since_last

        print(f"Frame at {current_time:.1f}s | FPS: {fps:.1f}")

        # Reset counters
        fps_counter.last_time = current_time
        fps_counter.frame_count = 0


@entrypoint
def main(app: App) -> App:
    """Configure and return the app."""
    return (
        app.add_plugins(MinimalPlugins)
        .add_systems(Startup, spawn_entities)
        .add_systems(Update, (update_positions, print_stats))
    )


if __name__ == "__main__":
    main().run()
