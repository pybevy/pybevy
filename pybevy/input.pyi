"""Input handling for PyBevy - keyboard, mouse, and gamepad input."""

from enum import Enum
from typing import ClassVar, Literal

from pybevy.app import App, Plugin
from pybevy.ecs import Component, Entity, Message, Resource
from pybevy.math import Vec2

class InputPlugin(Plugin):
    """Plugin that provides input handling (keyboard, mouse, gamepad, touch).

    This plugin initializes input resources like AccumulatedMouseMotion.
    """
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class KeyCode(Enum):
    """Keyboard key codes for input detection."""

    # Function keys
    F1 = ...
    F2 = ...
    F3 = ...
    F4 = ...
    F5 = ...
    F6 = ...
    F7 = ...
    F8 = ...
    F9 = ...
    F10 = ...
    F11 = ...
    F12 = ...

    # Modifier keys
    ShiftLeft = ...
    ShiftRight = ...
    ControlLeft = ...
    ControlRight = ...
    AltLeft = ...
    AltRight = ...
    SuperLeft = ...  # Windows/Command key
    SuperRight = ...

    # Common keys
    Space = ...
    Enter = ...
    Escape = ...
    Backspace = ...
    Tab = ...
    Delete = ...
    Insert = ...
    Home = ...
    End = ...
    PageUp = ...
    PageDown = ...
    CapsLock = ...
    ScrollLock = ...
    NumLock = ...

    # Punctuation keys
    Backquote = ...
    Backslash = ...
    BracketLeft = ...
    BracketRight = ...
    Comma = ...
    Equal = ...
    Minus = ...
    Period = ...
    Quote = ...
    Semicolon = ...
    Slash = ...

    # Arrow keys
    ArrowUp = ...
    ArrowDown = ...
    ArrowLeft = ...
    ArrowRight = ...

    # Letters
    KeyA = ...
    KeyB = ...
    KeyC = ...
    KeyD = ...
    KeyE = ...
    KeyF = ...
    KeyG = ...
    KeyH = ...
    KeyI = ...
    KeyJ = ...
    KeyK = ...
    KeyL = ...
    KeyM = ...
    KeyN = ...
    KeyO = ...
    KeyP = ...
    KeyQ = ...
    KeyR = ...
    KeyS = ...
    KeyT = ...
    KeyU = ...
    KeyV = ...
    KeyW = ...
    KeyX = ...
    KeyY = ...
    KeyZ = ...

    # Numbers
    Digit0 = ...
    Digit1 = ...
    Digit2 = ...
    Digit3 = ...
    Digit4 = ...
    Digit5 = ...
    Digit6 = ...
    Digit7 = ...
    Digit8 = ...
    Digit9 = ...

class ButtonInput(Resource):
    """
    Tracks the state of keyboard keys - whether they're pressed, just pressed, or just released.

    This resource is automatically provided as a system parameter and should not be
    instantiated directly.

    Example:
        ```python
        def handle_input_system(input: ButtonInput) -> None:
            if input.just_pressed(KeyCode.Space):
                print("Space bar was just pressed!")

            if input.pressed(KeyCode.ShiftLeft):
                print("Left shift is being held")

            if input.just_released(KeyCode.Escape):
                print("Escape was just released")
        ```
    """

    def __init__(self) -> None:
        """Create a new ButtonInput instance (typically done internally)."""

    def just_pressed(self, input: KeyCode) -> bool:
        """
        Returns true if the key was just pressed this frame.

        Args:
            input: The key code to check

        Returns:
            True if the key was just pressed this frame, False otherwise
        """

    def just_released(self, input: KeyCode) -> bool:
        """
        Returns true if the key was just released this frame.

        Args:
            input: The key code to check

        Returns:
            True if the key was just released this frame, False otherwise
        """

    def pressed(self, input: KeyCode) -> bool:
        """
        Returns true if the key is currently held down.

        Args:
            input: The key code to check

        Returns:
            True if the key is currently pressed, False otherwise
        """

    def any_just_pressed(self, inputs: list[KeyCode]) -> bool:
        """
        Returns true if any of the keys were just pressed this frame.

        Args:
            inputs: List of key codes to check

        Returns:
            True if any key in the list was just pressed
        """

    def any_pressed(self, inputs: list[KeyCode]) -> bool:
        """
        Returns true if any of the keys are currently pressed.

        Args:
            inputs: List of key codes to check

        Returns:
            True if any key in the list is currently pressed
        """

    def all_pressed(self, inputs: list[KeyCode]) -> bool:
        """
        Returns true if all of the keys are currently pressed.

        Args:
            inputs: List of key codes to check

        Returns:
            True if all keys in the list are currently pressed
        """

    def get_just_pressed(self) -> list[KeyCode]:
        """
        Get all keys that were just pressed this frame.

        Returns:
            List of KeyCodes that were just pressed
        """

    def get_pressed(self) -> list[KeyCode]:
        """
        Get all keys that are currently pressed.

        Returns:
            List of KeyCodes that are currently pressed
        """

    def get_just_released(self) -> list[KeyCode]:
        """
        Get all keys that were just released this frame.

        Returns:
            List of KeyCodes that were just released
        """

    def any_just_released(self, inputs: list[KeyCode]) -> bool:
        """
        Returns true if any of the keys were just released this frame.

        Args:
            inputs: List of key codes to check

        Returns:
            True if any key in the list was just released
        """

    def all_just_pressed(self, inputs: list[KeyCode]) -> bool:
        """
        Returns true if all of the keys were just pressed this frame.

        Args:
            inputs: List of key codes to check

        Returns:
            True if all keys in the list were just pressed
        """

    def all_just_released(self, inputs: list[KeyCode]) -> bool:
        """
        Returns true if all of the keys were just released this frame.

        Args:
            inputs: List of key codes to check

        Returns:
            True if all keys in the list were just released
        """

class MouseButton:
    """Mouse button codes for input detection."""

    class Left(MouseButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Right(MouseButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Middle(MouseButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Back(MouseButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Forward(MouseButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Other(MouseButton):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

class MouseInput(Resource):
    """
    Tracks the state of mouse buttons - whether they're pressed, just pressed, or just released.

    This resource is automatically provided as a system parameter and should not be
    instantiated directly.

    Example:
        ```python
        def handle_mouse_system(mouse: Res[MouseInput]) -> None:
            if mouse.just_pressed(MouseButton.Left()):
                print("Left mouse button was just pressed!")

            if mouse.pressed(MouseButton.Right()):
                print("Right mouse button is being held")

            if mouse.just_released(MouseButton.Middle()):
                print("Middle mouse button was just released")
        ```
    """

    def __init__(self) -> None:
        """Create a new MouseInput instance (typically done internally)."""

    def just_pressed(self, button: MouseButton) -> bool:
        """
        Returns true if the button was just pressed this frame.

        Args:
            button: The mouse button to check

        Returns:
            True if the button was just pressed this frame, False otherwise
        """

    def just_released(self, button: MouseButton) -> bool:
        """
        Returns true if the button was just released this frame.

        Args:
            button: The mouse button to check

        Returns:
            True if the button was just released this frame, False otherwise
        """

    def pressed(self, button: MouseButton) -> bool:
        """
        Returns true if the button is currently held down.

        Args:
            button: The mouse button to check

        Returns:
            True if the button is currently pressed, False otherwise
        """

    def any_just_pressed(self, buttons: list[MouseButton]) -> bool:
        """
        Returns true if any of the buttons were just pressed this frame.

        Args:
            buttons: List of mouse buttons to check

        Returns:
            True if any button in the list was just pressed
        """

    def any_pressed(self, buttons: list[MouseButton]) -> bool:
        """
        Returns true if any of the buttons are currently pressed.

        Args:
            buttons: List of mouse buttons to check

        Returns:
            True if any button in the list is currently pressed
        """

    def all_pressed(self, buttons: list[MouseButton]) -> bool:
        """
        Returns true if all of the buttons are currently pressed.

        Args:
            buttons: List of mouse buttons to check

        Returns:
            True if all buttons in the list are currently pressed
        """

    def get_just_pressed(self) -> list[MouseButton]:
        """
        Get all buttons that were just pressed this frame.

        Returns:
            List of MouseButtons that were just pressed
        """

    def get_pressed(self) -> list[MouseButton]:
        """
        Get all buttons that are currently pressed.

        Returns:
            List of MouseButtons that are currently pressed
        """

    def get_just_released(self) -> list[MouseButton]:
        """
        Get all buttons that were just released this frame.

        Returns:
            List of MouseButtons that were just released
        """

class ButtonState:
    """State of a button (pressed or released)."""

    @staticmethod
    def Pressed() -> ButtonState: ...
    @staticmethod
    def Released() -> ButtonState: ...
    def is_pressed(self) -> bool:
        """Returns true if this state is Pressed."""

class KeyboardInput(Message):
    """
    Keyboard input event message.

    Contains information about a key press or release event.
    Use with MessageReader to receive keyboard events.

    Example:
        ```python
        def handle_keys(reader: MessageReader[KeyboardInput]) -> None:
            for event in reader:
                if event.state == ButtonState.Pressed():
                    print(f"Key pressed: {event.key_code}")
        ```
    """

    def __init__(
        self,
        key_code: KeyCode,
        state: ButtonState,
        *,
        shift: bool = False,
        ctrl: bool = False,
        alt: bool = False,
        super_key: bool = False,
        repeat: bool = False,
        logical_key: str | None = None,
        text: str | None = None,
        window: Entity = ...,
    ) -> None: ...
    @property
    def key_code(self) -> KeyCode:
        """The key that was pressed or released."""

    @property
    def state(self) -> ButtonState:
        """Whether the key was pressed or released."""

    @property
    def logical_key(self) -> str | None:
        """Logical key representation (if available)."""

    @property
    def shift(self) -> bool:
        """Whether shift key is held."""

    @property
    def ctrl(self) -> bool:
        """Whether ctrl key is held."""

    @property
    def alt(self) -> bool:
        """Whether alt key is held."""

    @property
    def super_key(self) -> bool:
        """Whether super/command/windows key is held."""

    @property
    def repeat(self) -> bool:
        """Whether this is a repeated key event (key held down)."""

    @property
    def text(self) -> str | None:
        """
        The text produced by this keypress.

        Returns None if this keypress cannot be interpreted as text.
        """

    @property
    def window(self) -> Entity:
        """The window entity this event was received on."""

class MouseButtonInput(Message):
    """
    Mouse button input event message.

    Contains information about a mouse button press or release event.
    Use with MessageReader to receive mouse button events.

    Example:
        ```python
        def handle_clicks(reader: MessageReader[MouseButtonInput]) -> None:
            for event in reader:
                if event.state == ButtonState.Pressed():
                    print(f"Mouse button pressed: {event.button}")
        ```
    """

    def __init__(
        self, button: MouseButton, state: ButtonState, window: Entity = ...
    ) -> None: ...
    @property
    def button(self) -> MouseButton:
        """The mouse button that was pressed or released."""

    @property
    def state(self) -> ButtonState:
        """Whether the button was pressed or released."""

    @property
    def window(self) -> Entity:
        """The window entity this event was received on."""

class MouseMotion(Message):
    """
    Mouse motion event message.

    Contains information about mouse cursor movement.
    Use with MessageReader to receive mouse motion events.

    Example:
        ```python
        def handle_mouse_move(reader: MessageReader[MouseMotion]) -> None:
            for event in reader:
                print(f"Mouse moved: ({event.delta.x}, {event.delta.y})")
        ```
    """

    def __init__(self, delta: Vec2) -> None: ...
    @property
    def delta(self) -> Vec2:
        """Mouse movement delta as a Vec2."""

class MouseScrollUnit:
    """
    The scroll unit for a mouse wheel event.

    Describes how a value of a MouseWheel event has to be interpreted.
    The value can either be interpreted as the amount of lines or the amount of pixels to scroll.
    """

    Line: MouseScrollUnit
    """The line scroll unit - delta corresponds to lines/rows to scroll."""
    Pixel: MouseScrollUnit
    """The pixel scroll unit - delta corresponds to pixels to scroll."""

class MouseWheel(Message):
    """
    Mouse wheel scroll event message.

    Contains information about mouse wheel scrolling.
    Use with MessageReader to receive scroll events.

    Example:
        ```python
        def handle_scroll(reader: MessageReader[MouseWheel]) -> None:
            for event in reader:
                print(f"Scroll: ({event.x}, {event.y}) unit: {event.unit}")
        ```
    """

    def __init__(
        self, x: float, y: float, unit: MouseScrollUnit = ..., window: Entity = ...
    ) -> None: ...
    @property
    def x(self) -> float:
        """Horizontal scroll amount."""

    @property
    def y(self) -> float:
        """Vertical scroll amount."""

    @property
    def unit(self) -> MouseScrollUnit:
        """The scroll unit (Line or Pixel)."""

    @property
    def window(self) -> Entity:
        """The window entity this event was received on."""

class GamepadButton:
    """
    Gamepad button codes for input detection.

    Uses cardinal directions for face buttons (matching Bevy):
    - South: A button on Xbox, Cross on PlayStation
    - East: B button on Xbox, Circle on PlayStation
    - North: Y button on Xbox, Triangle on PlayStation
    - West: X button on Xbox, Square on PlayStation
    """

    class South(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class East(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class North(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class West(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class C(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class Z(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class LeftTrigger(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class LeftTrigger2(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class RightTrigger(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class RightTrigger2(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class Select(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class Start(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class Mode(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class LeftThumb(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class RightThumb(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class DPadUp(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class DPadDown(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class DPadLeft(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class DPadRight(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class Other(GamepadButton):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...
    @staticmethod
    def all() -> list[GamepadButton]:
        """Returns a list of all standard gamepad buttons (excluding Other)."""

class GamepadAxis:
    """Gamepad axis codes for analog input detection."""

    class LeftStickX(GamepadAxis):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class LeftStickY(GamepadAxis):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class LeftZ(GamepadAxis):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class RightStickX(GamepadAxis):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class RightStickY(GamepadAxis):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class RightZ(GamepadAxis):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...
    class Other(GamepadAxis):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...
    @staticmethod
    def all() -> list[GamepadAxis]:
        """Returns a list of all standard gamepad axes (excluding Other)."""

class GamepadInput:
    """
    Represents a gamepad input which can be either an axis or a button.

    Used for generic gamepad input operations where the input type may vary.
    """

    class Axis(GamepadInput):
        __match_args__: ClassVar[tuple[Literal["axis"]]]
        axis: GamepadAxis
        def __init__(self, axis: GamepadAxis) -> None: ...

    class Button(GamepadInput):
        __match_args__: ClassVar[tuple[Literal["button"]]]
        button: GamepadButton
        def __init__(self, button: GamepadButton) -> None: ...

class Gamepad(Component):
    """
    Gamepad component that tracks button and axis state for a connected gamepad.

    Query for entities with this component to access gamepad input state.

    Example:
        ```python
        def handle_gamepad(query: Query[Gamepad]) -> None:
            for gamepad in query:
                if gamepad.just_pressed(GamepadButton.South()):
                    print("A/Cross button pressed!")

                left_x = gamepad.get_axis(GamepadAxis.LeftStickX())
                if left_x is not None:
                    print(f"Left stick X: {left_x}")
        ```
    """

    def just_pressed(self, button_type: GamepadButton) -> bool:
        """Returns true if the button was just pressed this frame."""

    def just_released(self, button_type: GamepadButton) -> bool:
        """Returns true if the button was just released this frame."""

    def pressed(self, button_type: GamepadButton) -> bool:
        """Returns true if the button is currently held down."""

    def get_button(self, button: GamepadButton) -> float | None:
        """Get the analog value of a button (0.0 to 1.0), or None if not available."""

    def get_axis(self, axis: GamepadAxis) -> float | None:
        """Get the value of an axis (-1.0 to 1.0), or None if not available."""

    def get_button_unclamped(self, button: GamepadButton) -> float | None:
        """Get the unclamped analog value of a button (may be outside -1.0 to 1.0)."""

    def get_axis_unclamped(self, axis: GamepadAxis) -> float | None:
        """Get the unclamped value of an axis (may be outside -1.0 to 1.0)."""

    def get(self, input: GamepadInput) -> float | None:
        """Get the analog value of a GamepadInput (axis or button), clamped to [-1.0, 1.0]."""

    def get_unclamped(self, input: GamepadInput) -> float | None:
        """Get the unclamped analog value of a GamepadInput (axis or button)."""

    def get_analog_axes(self) -> list[GamepadInput]:
        """Get all analog inputs (axes and buttons) that have values."""

    def get_pressed(self) -> list[GamepadButton]:
        """Get all buttons that are currently pressed."""

    def get_just_pressed(self) -> list[GamepadButton]:
        """Get all buttons that were just pressed this frame."""

    def get_just_released(self) -> list[GamepadButton]:
        """Get all buttons that were just released this frame."""

    def left_stick(self) -> Vec2:
        """Returns the left analog stick as a Vec2 (x, y)."""

    def right_stick(self) -> Vec2:
        """Returns the right analog stick as a Vec2 (x, y)."""

    def dpad(self) -> Vec2:
        """Returns the directional pad as a Vec2 (x: left/right, y: up/down)."""

    def any_pressed(self, button_inputs: list[GamepadButton]) -> bool:
        """Returns true if any of the buttons are currently pressed."""

    def all_pressed(self, button_inputs: list[GamepadButton]) -> bool:
        """Returns true if all of the buttons are currently pressed."""

    def any_just_pressed(self, button_inputs: list[GamepadButton]) -> bool:
        """Returns true if any of the buttons were just pressed this frame."""

    def all_just_pressed(self, button_inputs: list[GamepadButton]) -> bool:
        """Returns true if all of the buttons were just pressed this frame."""

    def any_just_released(self, button_inputs: list[GamepadButton]) -> bool:
        """Returns true if any of the buttons were just released this frame."""

    def all_just_released(self, button_inputs: list[GamepadButton]) -> bool:
        """Returns true if all of the buttons were just released this frame."""

    def vendor_id(self) -> int | None:
        """Returns the USB vendor ID as assigned by the USB-IF, if available."""

    def product_id(self) -> int | None:
        """Returns the USB product ID as assigned by the vendor, if available."""

class GamepadButtonChanged(Message):
    """
    Gamepad button event message.

    Contains information about gamepad button changes with analog value.
    Use with MessageReader to receive gamepad button events.

    Example:
        ```python
        def handle_gamepad(reader: MessageReader[GamepadButtonChanged]) -> None:
            for event in reader:
                print(f"Button {event.button} value: {event.value}")
        ```
    """

    def __init__(self, button: GamepadButton, value: float) -> None: ...
    @property
    def button(self) -> GamepadButton:
        """The gamepad button that changed."""

    @property
    def value(self) -> float:
        """Analog value of the button (0.0 to 1.0)."""

class GamepadAxisChanged(Message):
    """
    Gamepad axis event message.

    Contains information about gamepad axis changes.
    Use with MessageReader to receive gamepad axis events.

    Example:
        ```python
        def handle_gamepad(reader: MessageReader[GamepadAxisChanged]) -> None:
            for event in reader:
                print(f"Axis {event.axis} value: {event.value}")
        ```
    """

    def __init__(self, axis: GamepadAxis, value: float) -> None: ...
    @property
    def axis(self) -> GamepadAxis:
        """The gamepad axis that changed."""

    @property
    def value(self) -> float:
        """Axis value (-1.0 to 1.0)."""

class GamepadConnection(Message):
    """
    Gamepad connection event message.

    Contains information about gamepad connection/disconnection with device metadata.
    Use with MessageReader to receive gamepad connection events.

    Example:
        ```python
        def handle_gamepad(reader: MessageReader[GamepadConnection]) -> None:
            for event in reader:
                if event.connected:
                    print(f"Gamepad connected: {event.name}")
                    print(f"Vendor ID: {event.vendor_id}, Product ID: {event.product_id}")
                else:
                    print("Gamepad disconnected")
        ```
    """

    def __init__(
        self,
        connected: bool,
        name: str | None = None,
        vendor_id: int | None = None,
        product_id: int | None = None,
    ) -> None: ...
    @property
    def connected(self) -> bool:
        """Whether the gamepad is connected."""

    @property
    def name(self) -> str | None:
        """
        The name of the gamepad, if connected.

        This name is generally defined by the OS.
        Example: "HID-compliant game controller" on Windows.
        """

    @property
    def vendor_id(self) -> int | None:
        """The USB vendor ID as assigned by the USB-IF, if available."""

    @property
    def product_id(self) -> int | None:
        """The USB product ID as assigned by the vendor, if available."""

class TouchPhase:
    """
    Touch phase enum - describes the current state of a touch.

    Variants:
        Started: A finger started to touch the touchscreen
        Moved: A finger moved over the touchscreen
        Ended: A finger stopped touching the touchscreen
        Canceled: The system canceled tracking (window lost focus, etc.)
    """

    Started: TouchPhase
    Moved: TouchPhase
    Ended: TouchPhase
    Canceled: TouchPhase

class TouchInput(Message):
    """
    Touch input event message.

    Contains information about touch screen interactions.
    Use with MessageReader to receive touch events.

    Example:
        ```python
        def handle_touch(reader: MessageReader[TouchInput]) -> None:
            for event in reader:
                if event.phase == TouchPhase.Started:
                    print(f"Touch started at {event.position}")
                elif event.phase == TouchPhase.Moved:
                    print(f"Touch moved to {event.position}")
                elif event.phase == TouchPhase.Ended:
                    print(f"Touch ended at {event.position}")
        ```
    """

    def __init__(
        self,
        phase: TouchPhase,
        position: Vec2,
        id: int,
        force: float | None = None,
        window: Entity = ...,
    ) -> None: ...
    @property
    def phase(self) -> TouchPhase:
        """The phase of the touch event."""

    @property
    def position(self) -> Vec2:
        """The position of the touch in window coordinates."""

    @property
    def id(self) -> int:
        """
        Unique identifier for this touch/finger.

        Different fingers will have different IDs, allowing multi-touch tracking.
        """

    @property
    def force(self) -> float | None:
        """
        Optional pressure data for pressure-sensitive touchscreens.

        Returns a value between 0.0 and 1.0, or None if not supported.
        """

    @property
    def window(self) -> Entity:
        """The window entity this event was received on."""

class AccumulatedMouseMotion(Resource):
    """Resource that accumulates mouse motion delta per frame.

    This resource tracks the total mouse movement that occurred during the current
    frame, providing a convenient way to access accumulated motion for camera controls,
    drag operations, and other mouse-based interactions.

    The delta values are reset each frame, so they represent only the current frame's motion.
    """

    def __init__(self) -> None: ...
    @property
    def delta(self) -> Vec2:
        """Accumulated mouse movement this frame as a Vec2."""

class AccumulatedMouseScroll(Resource):
    """Resource that accumulates mouse scroll delta per frame."""

    def __init__(self, unit: MouseScrollUnit = ...) -> None: ...
    @property
    def delta(self) -> Vec2:
        """Accumulated scroll this frame as a Vec2."""

    @property
    def unit(self) -> MouseScrollUnit:
        """The scroll unit (Line or Pixel)."""

class GamepadRumbleIntensity:
    """Gamepad rumble/haptic intensity settings."""

    MAX: GamepadRumbleIntensity
    WEAK_MAX: GamepadRumbleIntensity
    STRONG_MAX: GamepadRumbleIntensity

    def __init__(
        self,
        strong_motor: float = 1.0,
        weak_motor: float = 1.0,
    ) -> None: ...
    @property
    def strong_motor(self) -> float:
        """Intensity of the strong (low-frequency) motor (0.0-1.0)."""

    @property
    def weak_motor(self) -> float:
        """Intensity of the weak (high-frequency) motor (0.0-1.0)."""

class PinchGesture(Message):
    """Two-finger pinch gesture event (macOS/iOS only)."""

    def __init__(self, value: float) -> None: ...
    @property
    def value(self) -> float:
        """The pinch delta. Positive = magnify, negative = shrink."""

class RotationGesture(Message):
    """Two-finger rotation gesture event (macOS/iOS only)."""

    def __init__(self, value: float) -> None: ...
    @property
    def value(self) -> float:
        """The rotation delta in radians. Positive = counterclockwise."""

class DoubleTapGesture(Message):
    """Double tap gesture event (macOS/iOS only)."""

    def __init__(self) -> None: ...

class PanGesture(Message):
    """Pan gesture event."""

    def __init__(self, x: float, y: float) -> None: ...
    @property
    def x(self) -> float:
        """Horizontal pan delta."""

    @property
    def y(self) -> float:
        """Vertical pan delta."""

    @property
    def delta(self) -> Vec2:
        """Pan delta as a Vec2."""

class GamepadButtonStateChanged(Message):
    """Gamepad button state change event."""

    def __init__(self, button: GamepadButton, state: ButtonState) -> None: ...
    @property
    def button(self) -> GamepadButton:
        """The gamepad button that changed state."""

    @property
    def state(self) -> ButtonState:
        """The new state of the button."""

class GamepadEvent:
    """Unified gamepad event (connection, button, or axis).

    This is a PyO3 complex enum with Connection, Button, and Axis variants.
    Use pattern matching to handle different event types.
    """

    class Connection(GamepadEvent):
        __match_args__: ClassVar[
            tuple[
                Literal["connected"],
                Literal["name"],
                Literal["vendor_id"],
                Literal["product_id"],
            ]
        ]
        connected: bool
        name: str | None
        vendor_id: int | None
        product_id: int | None

        def __init__(
            self,
            connected: bool,
            name: str | None = None,
            vendor_id: int | None = None,
            product_id: int | None = None,
        ) -> None: ...

    class Button(GamepadEvent):
        __match_args__: ClassVar[tuple[Literal["button"], Literal["value"]]]
        button: GamepadButton
        value: float
        def __init__(self, button: GamepadButton, value: float) -> None: ...

    class Axis(GamepadEvent):
        __match_args__: ClassVar[tuple[Literal["axis"], Literal["value"]]]
        axis: GamepadAxis
        value: float
        def __init__(self, axis: GamepadAxis, value: float) -> None: ...

class KeyboardFocusLost(Message):
    """
    Keyboard focus lost event message.

    Triggered when the window loses keyboard focus. This is useful for pausing
    input processing or releasing currently pressed keys to avoid stuck key states.

    Example:
        ```python
        def handle_focus_lost(reader: MessageReader[KeyboardFocusLost]) -> None:
            for event in reader:
                print("Keyboard focus lost - pausing input")
                # Release all pressed keys or pause game
        ```
    """

    def __init__(self) -> None: ...

class GamepadRumbleRequest(Message):
    """
    Gamepad rumble/haptic feedback request message.

    Send this message to request haptic feedback on connected gamepads.
    Gamepads have two motors: strong (low-frequency) and weak (high-frequency).

    Example:
        ```python
        def trigger_rumble(writer: MessageWriter[GamepadRumbleRequest]) -> None:
            # Strong rumble on both motors for 0.5 seconds
            writer.send(GamepadRumbleRequest(duration_secs=0.5))

            # Custom motor intensities
            writer.send(GamepadRumbleRequest(
                duration_secs=0.3,
                strong_motor=1.0,
                weak_motor=0.5
            ))
        ```
    """

    def __init__(
        self,
        duration_secs: float,
        strong_motor: float = 1.0,
        weak_motor: float = 1.0,
        gamepad: Entity = ...,
    ) -> None: ...
    @property
    def duration_secs(self) -> float:
        """Duration of the rumble effect in seconds."""

    @property
    def strong_motor(self) -> float:
        """Intensity of the strong (low-frequency) motor (0.0-1.0)."""

    @property
    def weak_motor(self) -> float:
        """Intensity of the weak (high-frequency) motor (0.0-1.0)."""

    def gamepad(self) -> Entity:
        """Get the Entity associated with this request."""

class Touch:
    """
    A single touch input with position, force, and movement tracking.

    Tracks a finger's position and movement across the touchscreen, including
    starting position, previous position, and optional pressure data.
    """

    def __init__(self, id: int, position: Vec2) -> None: ...
    @property
    def id(self) -> int:
        """Unique identifier for this touch/finger."""

    @property
    def position(self) -> Vec2:
        """Current position of the touch."""

    @property
    def start_position(self) -> Vec2:
        """Position where the touch first made contact."""

    @property
    def previous_position(self) -> Vec2:
        """Position of the touch in the previous frame."""

    @property
    def force(self) -> float | None:
        """
        Current pressure/force of the touch, if supported.

        Normalized to 0.0-1.0 range. None if pressure sensing not available.
        """

    @property
    def start_force(self) -> float | None:
        """Pressure/force when the touch first made contact."""

    @property
    def previous_force(self) -> float | None:
        """Pressure/force in the previous frame."""

    def delta(self) -> Vec2:
        """Get the movement delta between current and previous position."""

    def distance(self) -> Vec2:
        """Get the total distance moved from start position to current position."""

class Touches(Resource):
    """
    Multi-touch input state tracking resource.

    Manages all active touches and provides queries for touch state changes.
    Automatically updated by the InputPlugin from touch screen events.

    Example:
        ```python
        def handle_touches(touches: Res[Touches]) -> None:
            # Check for new touches
            if touches.any_just_pressed():
                for touch in touches.iter_just_pressed():
                    print(f"New touch {touch.id} at {touch.position}")

            # Process active touches
            for touch in touches.iter():
                delta = touch.delta()
                print(f"Touch {touch.id} moved {delta.x}, {delta.y}")

            # Check for released touches
            for touch in touches.iter_just_released():
                distance = touch.distance()
                print(f"Touch {touch.id} released after {distance.length()} pixels")
        ```
    """

    def __init__(self) -> None: ...
    def any_just_pressed(self) -> bool:
        """Returns true if any touch was just started this frame."""

    def any_just_released(self) -> bool:
        """Returns true if any touch was just released this frame."""

    def any_just_canceled(self) -> bool:
        """Returns true if any touch was just canceled this frame."""

    def just_pressed(self, id: int) -> bool:
        """Returns true if the touch with given ID was just started."""

    def just_released(self, id: int) -> bool:
        """Returns true if the touch with given ID was just released."""

    def just_canceled(self, id: int) -> bool:
        """Returns true if the touch with given ID was just canceled."""

    def get_pressed(self, id: int) -> Touch | None:
        """Get touch data for a currently pressed touch by ID."""

    def get_released(self, id: int) -> Touch | None:
        """Get touch data for a just-released touch by ID."""

    def iter(self) -> list[Touch]:
        """Get all currently pressed touches."""

    def iter_just_pressed(self) -> list[Touch]:
        """Get all touches that were just started this frame."""

    def iter_just_released(self) -> list[Touch]:
        """Get all touches that were just released this frame."""

    def iter_just_canceled(self) -> list[Touch]:
        """Get all touches that were just canceled this frame."""

    def first_pressed_position(self) -> Vec2 | None:
        """Get the position of the first currently pressed touch, if any."""

    def clear(self) -> None:
        """Clears the just_pressed, just_released, and just_canceled data."""

    def clear_just_pressed(self, id: int) -> bool:
        """Clears the just_pressed state for a touch and returns True if it was just pressed."""

    def clear_just_released(self, id: int) -> bool:
        """Clears the just_released state for a touch and returns True if it was just released."""

    def clear_just_canceled(self, id: int) -> bool:
        """Clears the just_canceled state for a touch and returns True if it was just canceled."""

    def release(self, id: int) -> None:
        """Register a release for a given touch input."""

    def release_all(self) -> None:
        """Registers a release for all currently pressed touch inputs."""

    def reset_all(self) -> None:
        """Clears all touch data: pressed, just_pressed, just_released, and just_canceled."""

class ButtonSettings:
    """Button press/release threshold settings.

    Controls when a button is considered pressed or released based on analog values.
    """

    def __init__(
        self, press_threshold: float = 0.75, release_threshold: float = 0.65
    ) -> None:
        """Create button settings.

        Args:
            press_threshold: Value above which button is pressed (0.0-1.0)
            release_threshold: Value below which button is released (0.0-1.0)

        Raises:
            ValueError: If thresholds are out of range or release > press
        """

    @property
    def press_threshold(self) -> float:
        """The threshold above which a button is considered pressed."""

    @property
    def release_threshold(self) -> float:
        """The threshold below which a button is considered released."""

    def is_pressed(self, value: float) -> bool:
        """Returns True if the button is considered pressed at the given value."""

    def is_released(self, value: float) -> bool:
        """Returns True if the button is considered released at the given value."""

class AxisSettings:
    """Axis deadzone and livezone settings.

    Controls how axis values are processed, including deadzones and livezones.
    """

    def __init__(
        self,
        livezone_lowerbound: float = -1.0,
        deadzone_lowerbound: float = -0.05,
        deadzone_upperbound: float = 0.05,
        livezone_upperbound: float = 1.0,
        threshold: float = 0.01,
    ) -> None:
        """Create axis settings.

        Args:
            livezone_lowerbound: Value below which inputs round to -1.0
            deadzone_lowerbound: Value above which negative inputs round to 0.0
            deadzone_upperbound: Value below which positive inputs round to 0.0
            livezone_upperbound: Value above which inputs round to 1.0
            threshold: Minimum change required to register input

        Raises:
            ValueError: If bounds are invalid
        """

    @property
    def livezone_upperbound(self) -> float:
        """Value above which inputs are rounded to 1.0."""

    @property
    def deadzone_upperbound(self) -> float:
        """Value below which positive inputs are rounded to 0.0."""

    @property
    def deadzone_lowerbound(self) -> float:
        """Value above which negative inputs are rounded to 0.0."""

    @property
    def livezone_lowerbound(self) -> float:
        """Value below which inputs are rounded to -1.0."""

    @property
    def threshold(self) -> float:
        """Minimum change required to register input."""

    def clamp(self, value: float) -> float:
        """Clamp a raw axis value according to settings."""

class ButtonAxisSettings:
    """Button axis settings for analog button values.

    Controls how analog button values are rounded.
    """

    def __init__(
        self, high: float = 0.95, low: float = 0.05, threshold: float = 0.01
    ) -> None:
        """Create button axis settings.

        Args:
            high: Value at which to round to 1.0
            low: Value at which to round to 0.0
            threshold: Threshold for change detection
        """

    @property
    def high(self) -> float:
        """The high value at which to round to 1.0."""

    @property
    def low(self) -> float:
        """The low value at which to round to 0.0."""

    @property
    def threshold(self) -> float:
        """The threshold for change detection."""

class GamepadSettings(Component):
    """Gamepad input settings component.

    Controls deadzone, livezone, and threshold settings for gamepad inputs.
    Attached to gamepad entities to customize their input behavior.
    """

    def __init__(self) -> None: ...
    def button_settings_for(self, button: GamepadButton) -> ButtonSettings:
        """Get button settings for a specific button."""

    def axis_settings_for(self, axis: GamepadAxis) -> AxisSettings:
        """Get axis settings for a specific axis."""

    def button_axis_settings_for(self, button: GamepadButton) -> ButtonAxisSettings:
        """Get button axis settings for a specific button."""

    @property
    def default_button_settings(self) -> ButtonSettings:
        """Get the default button settings."""

    @property
    def default_axis_settings(self) -> AxisSettings:
        """Get the default axis settings."""

    @property
    def default_button_axis_settings(self) -> ButtonAxisSettings:
        """Get the default button axis settings."""

    @property
    def button_settings(self) -> dict[GamepadButton, ButtonSettings]:
        """Get all custom button settings."""

    @property
    def axis_settings(self) -> dict[GamepadAxis, AxisSettings]:
        """Get all custom axis settings."""

    @property
    def button_axis_settings(self) -> dict[GamepadButton, ButtonAxisSettings]:
        """Get all custom button axis settings."""

# Type aliases for Bevy's Event suffix naming convention
GamepadAxisChangedEvent = GamepadAxisChanged
"""Alias for GamepadAxisChanged (Bevy naming convention)."""

GamepadButtonChangedEvent = GamepadButtonChanged
"""Alias for GamepadButtonChanged (Bevy naming convention)."""

GamepadButtonStateChangedEvent = GamepadButtonStateChanged
"""Alias for GamepadButtonStateChanged (Bevy naming convention)."""

GamepadConnectionEvent = GamepadConnection
"""Alias for GamepadConnection (Bevy naming convention)."""
