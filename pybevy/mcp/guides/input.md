# Input Guide

Handling keyboard, mouse, and gamepad input in PyBevy.

## Keyboard Input

Use `Res[ButtonInput]` to read keyboard state. **Important:** the type is `ButtonInput` (not generic `ButtonInput[KeyCode]`) — PyBevy bridges this as a concrete resource.

```python
from pybevy.input import ButtonInput, KeyCode
from pybevy.prelude import *

def movement_system(keyboard: Res[ButtonInput], time: Res[Time]) -> None:
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

### Common KeyCode Variants

| Category | Variants |
|----------|----------|
| Letters | `KeyCode.KeyA` … `KeyCode.KeyZ` |
| Digits | `KeyCode.Digit0` … `KeyCode.Digit9` |
| Arrows | `KeyCode.ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight` |
| Common | `KeyCode.Space`, `Enter`, `Escape`, `Backspace`, `Tab`, `Delete` |
| Modifiers | `KeyCode.ShiftLeft`, `ShiftRight`, `ControlLeft`, `ControlRight`, `AltLeft`, `AltRight` |
| Function | `KeyCode.F1` … `KeyCode.F12` |

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

For event-driven input (e.g. text input, modifier tracking), use `MessageReader[KeyboardInput]`:

```python
from pybevy.input import KeyboardInput, ButtonState

def key_events(reader: MessageReader[KeyboardInput]) -> None:
    for event in reader:
        if event.state == ButtonState.Pressed():
            print(f"Key pressed: {event.key_code}, shift={event.shift}, ctrl={event.ctrl}")
        if event.text:
            print(f"Text: {event.text}")
```

## Mouse Input

Mouse buttons use `Res[MouseInput]` (not `Res[ButtonInput]`). **Note:** `MouseButton` variants are constructor calls with parentheses, unlike `KeyCode` enum values.

```python
from pybevy.input import MouseInput, MouseButton
from pybevy.prelude import *

def click_system(mouse: Res[MouseInput]) -> None:
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

def camera_look(motion: Res[AccumulatedMouseMotion]) -> None:
    if motion.delta.x != 0.0 or motion.delta.y != 0.0:
        yaw = -motion.delta.x * 0.003
        pitch = -motion.delta.y * 0.003
        # Apply to camera transform...
```

### Mouse Scroll

```python
from pybevy.input import MouseWheel

def scroll_system(scroll: MessageReader[MouseWheel]) -> None:
    for event in scroll:
        if event.y != 0.0:
            print(f"scroll: {event.y}")
```

## Import Summary

| Type | Import |
|------|--------|
| `ButtonInput`, `KeyCode` | `from pybevy.input import ButtonInput, KeyCode` |
| `MouseInput`, `MouseButton` | `from pybevy.input import MouseInput, MouseButton` |
| `AccumulatedMouseMotion` | `from pybevy.input import AccumulatedMouseMotion` |
| `MouseWheel` | `from pybevy.input import MouseWheel` |
| `KeyboardInput`, `ButtonState` | `from pybevy.input import KeyboardInput, ButtonState` |

**Note:** `KeyCode` and `ButtonInput` are also re-exported from `pybevy.prelude`, so `from pybevy.prelude import *` covers keyboard input. Mouse types require explicit imports from `pybevy.input`.

## Gamepad Input

Gamepads are entity-based — each connected gamepad is an entity with a `Gamepad` component:

```python
from pybevy.input import Gamepad, GamepadButton, GamepadAxis

def gamepad_system(query: Query[Gamepad]) -> None:
    for gamepad in query:
        if gamepad.just_pressed(GamepadButton.South()):  # A / Cross
            print("jump!")

        # Analog sticks return Vec2
        stick = gamepad.left_stick()
        if abs(stick.x) > 0.1 or abs(stick.y) > 0.1:
            print(f"stick: {stick.x}, {stick.y}")

        # Triggers (analog 0.0–1.0)
        trigger = gamepad.get_button(GamepadButton.RightTrigger2())
        if trigger and trigger > 0.5:
            print("right trigger pulled")
```

See `pybevy.input` stubs for the full API including `GamepadAxis`, `GamepadSettings`, `Touches`, and gesture events.
