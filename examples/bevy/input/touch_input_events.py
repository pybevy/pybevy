"""Prints out all touch inputs.

Demonstrates:
- Receiving touch events via MessageReader
- TouchInput message with phase, position, ID, and force
- Different touch phases (Started, Moved, Ended, Canceled)
- Multi-touch tracking via unique touch IDs

Note: This example requires a touchscreen or touch input device.
On desktop systems without touch input, the example will run but won't receive any events.
"""

from pybevy.input import TouchInput
from pybevy.prelude import *


def touch_event_system(touch_inputs: MessageReader[TouchInput]) -> None:
    """Print all touch events to the console."""
    for touch_input in touch_inputs:
        print(f"Touch event: {touch_input}")


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Update, touch_event_system)


if __name__ == "__main__":
    main().run()
