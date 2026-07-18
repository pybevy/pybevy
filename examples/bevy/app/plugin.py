"""Demonstrates creation and registration of a custom plugin.

Plugins are the foundation of Bevy. They are scoped sets of components, resources, and systems
that provide specific functionality. This example shows how to create a simple plugin that
prints a message at regular intervals.

Demonstrates:
- Creating a custom Plugin class
- Plugin configuration via __init__ parameters
- Registering resources in plugin.build()
- Adding systems from within a plugin
- Using Timer for recurring actions
"""

from pybevy.prelude import *


@plugin
class PrintMessagePlugin(Plugin):
    """Plugin that prints a message at regular intervals.

    This demonstrates how to create configurable plugins with initialization parameters.
    """

    def __init__(self, wait_duration: float, message: str):
        """Initialize the plugin with configuration.

        Args:
            wait_duration: Seconds between message prints
            message: The message to print
        """
        super().__init__()
        self.wait_duration = wait_duration
        self.message = message

    def build(self, app: App) -> None:
        """Set up the plugin by adding resources and systems.

        This method is called when the plugin is added to the app.
        """
        state = PrintMessageState(
            message=self.message,
            timer=Timer(self.wait_duration, TimerMode.Repeating)
        )
        app.insert_resource(state)
        app.add_systems(Update, print_message_system)


@resource
class PrintMessageState(Resource):
    """Resource holding the plugin's state."""

    def __init__(self, message: str, timer: Timer):
        self.message = message
        self.timer = timer


def print_message_system(state: ResMut[PrintMessageState], time: Res[Time]) -> None:
    """Print the message when the timer finishes."""
    state.timer.tick(time.delta_secs())
    if state.timer.is_finished():
        print(state.message)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(PrintMessagePlugin(
            wait_duration=1.0,
            message="This is an example plugin"
        ))
    )


if __name__ == "__main__":
    main().run()
