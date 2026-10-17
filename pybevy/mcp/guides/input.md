# Input Guide

Handling keyboard, mouse, and gamepad input in PyBevy.

## Keyboard Input

Use `Res[ButtonInput[KeyCode]]` to read keyboard state.

```python
from pybevy.input import ButtonInput, KeyCode
from pybevy.prelude import *

@system
def movement_system(keyboard: Res[ButtonInput[KeyCode]], time: Res[Time]) -> None:
    dt = time.delta_secs()
    speed = 5.0

    # Held down (continuous)
    if keyboard.pressed(KeyCode.KeyW) or keyboard.pressed(KeyCode.ArrowUp):
        print("moving forward")
    if keyboard.pressed(KeyCode.KeyA) or keyboard.pressed(KeyCode.ArrowLeft):
        print("moving left")

    # Just pressed this frame (one-shot)
    if keyboard.just_pressed(KeyCode.Space):
        print("jump!")

    # Just released this frame
    if keyboard.just_released(KeyCode.Escape):
        print("escape released")
```

For a keyboard run condition, import `input_just_pressed` from
`pybevy.ecs.conditions` and use
`system(jump).run_if(input_just_pressed(KeyCode.Space))`, with `system` from
`pybevy.ecs`. The helper reads `Res[ButtonInput[KeyCode]]`; install
`InputPlugin` or the default plugin group to provide the keyboard resource.
It becomes false again when the next input update clears the press edge.

### Common KeyCode Variants

| Category | Variants |
|----------|----------|
| Letters | `KeyCode.KeyA` … `KeyCode.KeyZ` |
| Digits | `KeyCode.Digit0` … `KeyCode.Digit9` |
| Arrows | `KeyCode.ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight` |
| Common | `KeyCode.Space`, `Enter`, `Escape`, `Backspace`, `Tab`, `Delete` |
| Modifiers | `KeyCode.ShiftLeft`, `ShiftRight`, `ControlLeft`, `ControlRight`, `AltLeft`, `AltRight` |
| Function | `KeyCode.F1` … `KeyCode.F12` |

Platform-specific physical keys retain their native code:

```python
from pybevy.input import KeyCode, NativeKeyCode

key = KeyCode.Unidentified(NativeKeyCode.Xkb(42))
```

### Batch Queries

```python
# Any of these pressed?
if keyboard.any_pressed([KeyCode.KeyW, KeyCode.ArrowUp]):
    print("forward")

# All held simultaneously?
if keyboard.all_pressed([KeyCode.ControlLeft, KeyCode.KeyS]):
    print("ctrl+s")

# Get all keys pressed this frame
for key in keyboard.get_just_pressed():
    print(f"pressed: {key}")
```

### Keyboard Events (Message-Based)

For event-driven input, including text input, use `MessageReader[KeyboardInput]`; window events use `MessageReader[WindowEvent]`:

```python
from pybevy.input import ButtonState, Key, KeyboardInput
from pybevy.ecs import system

@system
def key_events(reader: MessageReader[KeyboardInput]) -> None:
    for event in reader:
        if event.state == ButtonState.Pressed():
            print(f"Key pressed: {event.key_code}")
        if event.logical_key == Key.Enter():
            print("Enter pressed")
        if event.text:
            print(f"Text: {event.text}")
```

As in Bevy, each `WindowEvent` variant wraps the message of the same name:

```python
from pybevy.ecs import MessageReader, system
from pybevy.input import KeyboardInput
from pybevy.window import WindowEvent

@system
def window_events(reader: MessageReader[WindowEvent]) -> None:
    for event in reader:
        match event:
            case WindowEvent.KeyboardInput(KeyboardInput(key_code=key)):
                print(f"Key: {key}")
            case WindowEvent.CursorMoved(moved):
                print(f"Cursor at: {moved.position}")
```

## Mouse Input

Mouse buttons use `Res[ButtonInput[MouseButton]]`, the same generic as the keyboard with a different button type. It resolves to the `MouseInput` class, which remains a valid spelling. **Note:** `MouseButton` variants are constructor calls with parentheses, unlike `KeyCode` enum values.

Compare event values with `event.button == MouseButton.Left()` and
`event.state == ButtonState.Pressed()`. Missing parentheses raise `TypeError`
with a constructor hint; replace its `...` with any required payload arguments.

```python
from pybevy.input import ButtonInput, MouseButton
from pybevy.prelude import *

@system
def click_system(mouse: Res[ButtonInput[MouseButton]]) -> None:
    # Note the parens: MouseButton.Left() not MouseButton.Left
    if mouse.just_pressed(MouseButton.Left()):
        print("left click")
    if mouse.pressed(MouseButton.Right()):
        print("right held")
    if mouse.just_released(MouseButton.Middle()):
        print("middle released")
```

### Mouse Motion

```python
from pybevy.input import AccumulatedMouseMotion
from pybevy.ecs import system

@system
def camera_look(motion: Res[AccumulatedMouseMotion]) -> None:
    if motion.delta.x != 0.0 or motion.delta.y != 0.0:
        yaw = -motion.delta.x * 0.003
        pitch = -motion.delta.y * 0.003
        # Apply to camera transform...
```

### Mouse Scroll

```python
from pybevy.input import MouseWheel
from pybevy.ecs import system

@system
def scroll_system(scroll: MessageReader[MouseWheel]) -> None:
    for event in scroll:
        if event.y != 0.0:
            print(f"scroll: {event.y}")
```

## Import Summary

| Type | Import |
|------|--------|
| `ButtonInput[KeyCode]`, `KeyCode`, `NativeKeyCode` | `from pybevy.input import ButtonInput, KeyCode, NativeKeyCode` |
| `ButtonInput[MouseButton]` | `from pybevy.input import ButtonInput, MouseButton` |
| `AccumulatedMouseMotion` | `from pybevy.input import AccumulatedMouseMotion` |
| `MouseWheel` | `from pybevy.input import MouseWheel` |
| `KeyboardInput`, `ButtonState`, `Key` | `from pybevy.input import ButtonState, Key, KeyboardInput` |

**Note:** `KeyCode` and `ButtonInput` are also re-exported from `pybevy.prelude`, so `from pybevy.prelude import *` covers keyboard input. `MouseButton` still needs an explicit import from `pybevy.input`.

## Gamepad Input

Gamepads are entity-based - each connected gamepad is an entity with a `Gamepad` component.
`MessageReader[GamepadEvent]` yields `Connection`, `Button`, or `Axis` variants.
Each variant's `value` returns an owned event payload, preserving its gamepad
entity and all event fields. Use the corresponding event wrapper when constructing
a variant, such as `GamepadEvent.Axis(GamepadAxisChangedEvent(entity, axis, value))`.
Payload reads are independent copies, not mutable views into the message.

```python
from pybevy.input import Gamepad, GamepadButton, GamepadAxis
from pybevy.ecs import system

@system
def gamepad_system(query: Query[Gamepad]) -> None:
    for gamepad in query:
        if gamepad.just_pressed(GamepadButton.South()):  # A / Cross
            print("jump!")

        # Analog sticks return Vec2
        stick = gamepad.left_stick()
        if abs(stick.x) > 0.1 or abs(stick.y) > 0.1:
            print(f"stick: {stick.x}, {stick.y}")

        # Triggers (analog 0.0–1.0)
        trigger = gamepad.get(GamepadButton.RightTrigger2())
        if trigger and trigger > 0.5:
            print("right trigger pulled")
```

See `pybevy.input` stubs for the full API including `GamepadAxis`, `GamepadSettings`, `Touches`, and gesture events.
`GamepadSettings.get_button_settings(button)`, `get_axis_settings(axis)`, and
`get_button_axis_settings(button)` return the configured value or its default.
The per-input mappings are read-only snapshots; assign a whole mapping through
`Query[Mut[GamepadSettings]]` to change them. `PanGesture` exposes its vector
through `value` (`value.x` and `value.y`), not separate coordinate aliases.

## Simulating Input

Input messages are writable, so a test or headless scene can drive the real
pipeline instead of faking state: `InputPlugin`'s own systems consume them in
`PreUpdate` and update `ButtonInput`, keeping `pressed` / `just_pressed` honest.
A message written during `Update` reaches `ButtonInput` on the next frame, and
releasing a key needs a matching `ButtonState.Released()` message.

```python
from pybevy.ecs import MessageWriter, system
from pybevy.input import ButtonState, Key, KeyboardInput, KeyCode

@system
def simulate_jump(writer: MessageWriter[KeyboardInput]) -> None:
    writer.write(
        KeyboardInput(
            key_code=KeyCode.Space,
            logical_key=Key.Space(),
            state=ButtonState.Pressed(),
        )
    )
```

Writable inputs: `KeyboardInput`, `MouseButtonInput`, `MouseMotion`, `MouseWheel`,
`TouchInput`, `GamepadConnectionEvent`, `RawGamepadEvent`, and
`CursorMoved` / `RequestRedraw` from `pybevy.window`. Synthetic `MouseWheel`
messages default to `TouchPhase.Moved` but may provide another phase. For gamepad
button and axis state, write `RawGamepadEvent.Button(RawGamepadButtonChangedEvent(gamepad, button, value))`
or `RawGamepadEvent.Axis(RawGamepadAxisChangedEvent(gamepad, axis, value))`. Bevy processes these
in `PreUpdate`; write them in `First` for same-frame state updates. The separate
`RawGamepadButtonChangedEvent` and `RawGamepadAxisChangedEvent` channels and the
processed `GamepadButtonChangedEvent`, `GamepadButtonStateChangedEvent`,
`GamepadAxisChangedEvent`, and `GamepadEvent` channels are writable but do not
drive `Gamepad` state. Connect a spawned entity with `GamepadConnectionEvent`
before sending button or axis input; a `RawGamepadEvent.Connection` by itself
does not insert the `Gamepad` component. `GamepadRumbleRequest` requests haptic
output.

```python
from pybevy.ecs import Commands, Entity, MessageWriter, Query, With, system
from pybevy.input import (
    Gamepad,
    GamepadButton,
    GamepadConnection,
    GamepadConnectionEvent,
    RawGamepadButtonChangedEvent,
    RawGamepadEvent,
)

@system  # Startup: the connection inserts Name and Gamepad on the entity
def connect_virtual_pad(
    commands: Commands, connect: MessageWriter[GamepadConnectionEvent]
) -> None:
    pad = commands.spawn().id()
    connected = GamepadConnection.Connected(name="Virtual pad", vendor_id=None, product_id=None)
    connect.write(GamepadConnectionEvent(pad, connected))

@system  # First: press the south button (A / Cross) for this frame's Gamepad state
def press_south(
    pads: Query[Entity, With[Gamepad]], raw: MessageWriter[RawGamepadEvent]
) -> None:
    for pad in pads:
        raw.write(RawGamepadEvent.Button(RawGamepadButtonChangedEvent(pad, GamepadButton.South(), 1.0)))
```

Button values pass Bevy's press threshold (0.75) and release threshold (0.65);
write `0.0` to release. Axis values within the 0.05 dead zone read as `0.0`.

Bevy's picking reads the canonical `WindowEvent` stream, not `CursorMoved` or
`MouseButtonInput`. To hover or click UI from a script, write pointer input as
`WindowEvent` variants, in `First` so picking sees them that frame:

```python
from pybevy.ecs import Entity, MessageWriter, Query, With, system
from pybevy.input import ButtonState, MouseButton, MouseButtonInput
from pybevy.math import Vec2
from pybevy.window import CursorMoved, PrimaryWindow, WindowEvent

@system
def click(windows: Query[Entity, With[PrimaryWindow]], events: MessageWriter[WindowEvent]) -> None:
    window = windows.single()
    events.write(WindowEvent.CursorMoved(CursorMoved(window=window, position=Vec2(200.0, 140.0))))
    events.write(WindowEvent.MouseButtonInput(
        MouseButtonInput(button=MouseButton.Left(), state=ButtonState.Pressed(), window=window)
    ))
```

`InputPlugin` processes `TouchInput` in `PreUpdate`; write in `First` for same-frame
`Touches` updates. `force` is a
`ForceTouch.Calibrated(force=1.0, max_possible_force=6.0, altitude_angle=None)` or
`ForceTouch.Normalized(value)`, or `None` without pressure data.
