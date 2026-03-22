"""Gamepad input example demonstrating state-based polling and event handling.

This example shows how to:
- Query for connected gamepads
- Read button state (pressed, just_pressed, just_released)
- Read analog stick values
- Read trigger values
- Handle gamepad connection/disconnection events
- Display real-time input values

Connect a gamepad to test this example.
"""

from pybevy.input import (
    Gamepad,
    GamepadAxis,
    GamepadAxisChanged,
    GamepadButton,
    GamepadButtonChanged,
    GamepadConnection,
)
from pybevy.prelude import *


@component
class InputDisplay(Component):
    """Marker component for entities that display input."""



def setup(commands: Commands) -> None:
    """Setup the scene."""
    # The gamepad entities are created automatically by Bevy when controllers connect
    # We just need to add a marker for display tracking
    commands.spawn(InputDisplay())
    print("Gamepad input example")
    print("=" * 60)
    print("Connect a gamepad and press buttons to see input values")
    print()


def display_gamepad_state(query: Query[tuple[Gamepad, Entity]]) -> None:
    """Display current state of all connected gamepads."""
    for gamepad, entity in query:
        # Check button states
        if gamepad.just_pressed(GamepadButton.South()):
            print(f"[Gamepad {entity}] South button (A/Cross) pressed!")

        if gamepad.just_pressed(GamepadButton.East()):
            print(f"[Gamepad {entity}] East button (B/Circle) pressed!")

        if gamepad.just_pressed(GamepadButton.North()):
            print(f"[Gamepad {entity}] North button (Y/Triangle) pressed!")

        if gamepad.just_pressed(GamepadButton.West()):
            print(f"[Gamepad {entity}] West button (X/Square) pressed!")

        if gamepad.just_pressed(GamepadButton.Start()):
            print(f"[Gamepad {entity}] Start button pressed!")

        if gamepad.just_pressed(GamepadButton.Select()):
            print(f"[Gamepad {entity}] Select button pressed!")

        # Check D-pad
        if gamepad.just_pressed(GamepadButton.DPadUp()):
            print(f"[Gamepad {entity}] D-Pad Up pressed!")

        if gamepad.just_pressed(GamepadButton.DPadDown()):
            print(f"[Gamepad {entity}] D-Pad Down pressed!")

        if gamepad.just_pressed(GamepadButton.DPadLeft()):
            print(f"[Gamepad {entity}] D-Pad Left pressed!")

        if gamepad.just_pressed(GamepadButton.DPadRight()):
            print(f"[Gamepad {entity}] D-Pad Right pressed!")

        # Check stick buttons
        if gamepad.just_pressed(GamepadButton.LeftThumb()):
            print(f"[Gamepad {entity}] Left stick button pressed!")

        if gamepad.just_pressed(GamepadButton.RightThumb()):
            print(f"[Gamepad {entity}] Right stick button pressed!")

        # Read analog sticks
        left_x = gamepad.get_axis(GamepadAxis.LeftStickX())
        left_y = gamepad.get_axis(GamepadAxis.LeftStickY())
        right_x = gamepad.get_axis(GamepadAxis.RightStickX())
        right_y = gamepad.get_axis(GamepadAxis.RightStickY())

        # Apply deadzone and display stick values
        deadzone = 0.15
        if left_x is not None and left_y is not None and (abs(left_x) > deadzone or abs(left_y) > deadzone):
            print(
                f"[Gamepad {entity}] Left Stick: ({left_x:.2f}, {left_y:.2f})"
            )

        if right_x is not None and right_y is not None and (abs(right_x) > deadzone or abs(right_y) > deadzone):
            print(
                f"[Gamepad {entity}] Right Stick: ({right_x:.2f}, {right_y:.2f})"
            )

        # Read triggers (analog buttons)
        left_trigger = gamepad.get_button(GamepadButton.LeftTrigger2())
        right_trigger = gamepad.get_button(GamepadButton.RightTrigger2())

        if left_trigger is not None and left_trigger > 0.1:
            print(f"[Gamepad {entity}] Left Trigger: {left_trigger:.2f}")

        if right_trigger is not None and right_trigger > 0.1:
            print(f"[Gamepad {entity}] Right Trigger: {right_trigger:.2f}")


def handle_button_events(reader: MessageReader[GamepadButtonChanged]) -> None:
    """Handle gamepad button change events."""
    for event in reader:
        if event.value > 0.5:
            print(f"[Event] Button {event.button} pressed (value: {event.value:.2f})")
        elif event.value < 0.1:
            print(f"[Event] Button {event.button} released")


def handle_axis_events(reader: MessageReader[GamepadAxisChanged]) -> None:
    """Handle gamepad axis change events."""
    for event in reader:
        # Only print significant axis changes to avoid spam
        if abs(event.value) > 0.2:
            print(f"[Event] Axis {event.axis} changed to {event.value:.2f}")


def handle_connection_events(reader: MessageReader[GamepadConnection]) -> None:
    """Handle gamepad connection/disconnection events."""
    for event in reader:
        if event.connected:
            print()
            print(" Gamepad connected!")
            print()
        else:
            print()
            print(" Gamepad disconnected")
            print()


def show_usage_every_5_seconds(time: Res[Time]) -> None:
    """Periodically show usage instructions."""
    elapsed = time.elapsed_secs()
    if elapsed > 0 and int(elapsed) % 5 == 0 and time.delta_secs() < 0.1:
        print()
        print(" Usage:")
        print("  - Press buttons to see button events")
        print("  - Move sticks to see analog values")
        print("  - Pull triggers to see trigger values")
        print("  - Use state-based polling for gameplay, events for UI feedback")
        print()


@entrypoint
def main(app: App) -> App:
    """Configure and run the gamepad input example."""
    return (
        app.add_plugins(DefaultPlugins)
        .add_message(GamepadButtonChanged)
        .add_message(GamepadAxisChanged)
        .add_message(GamepadConnection)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                display_gamepad_state,
                handle_button_events,
                handle_axis_events,
                handle_connection_events,
                show_usage_every_5_seconds,
            ),
        )
    )


if __name__ == "__main__":
    main().run()
