"""Fly Camera Plugin - Free-moving camera with WASD/arrow keys and mouse look.

Usage:
    from pybevy.contrib import FlyCameraPlugin

    app.add_plugins(FlyCameraPlugin())
"""

from dataclasses import dataclass

from ..app import App, Plugin, Update
from ..decorators import component, plugin, resource
from ..ecs import Component, MessageReader, Mut, Query, Res, ResMut, Resource
from ..input import (
    ButtonInput,
    ButtonState,
    KeyboardInput,
    KeyCode,
    MouseButton,
    MouseInput,
    MouseMotion,
)
from ..math import Quat, Vec3
from ..time import Time
from ..transform import Transform


@component
@dataclass
class FlyCamera(Component):
    """Component for fly camera controller.

    Attributes:
        move_speed: Movement speed in units per second (default: 10.0)
        sprint_multiplier: Speed multiplier when shift is held (default: 2.5)
        look_sensitivity: Mouse look sensitivity (default: 0.003)
        pitch: Current vertical rotation angle in radians (default: 0.0)
        yaw: Current horizontal rotation angle in radians (default: 0.0)
        max_pitch: Maximum pitch angle in radians (default: π/2 - 0.01)
    """

    move_speed: float = 10.0
    sprint_multiplier: float = 2.5
    look_sensitivity: float = 0.003
    pitch: float = 0.0
    yaw: float = 0.0
    max_pitch: float = 1.5607963267948966  # π/2 - 0.01


@resource
class FlyCameraState(Resource):
    """Resource for tracking fly camera state."""

    def __init__(self) -> None:
        self.right_mouse_pressed = False
        self.shift_pressed = False


def fly_camera_control_system(
    query: Query[tuple[Mut[Transform], Mut[FlyCamera]]],
    mouse_buttons: Res[MouseInput],
    keyboard_input: MessageReader[KeyboardInput],
    mouse_motion: MessageReader[MouseMotion],
    camera_state: ResMut[FlyCameraState],
) -> None:
    """System that handles fly camera controls.

    Controls:
        - WASD / Arrow keys: Move forward/left/backward/right
        - Q/E: Move down/up
        - Shift: Sprint (faster movement)
        - Right mouse + drag: Look around
    """
    # Track shift key state
    for event in keyboard_input:
        if event.key_code == KeyCode.ShiftLeft or event.key_code == KeyCode.ShiftRight:
            camera_state.shift_pressed = event.state == ButtonState.Pressed()

    # Track right mouse button state
    camera_state.right_mouse_pressed = mouse_buttons.pressed(MouseButton.Right())

    for transform, camera in query:
        # Mouse look (right button)
        if camera_state.right_mouse_pressed:
            for motion in mouse_motion:
                # Update yaw (horizontal rotation)
                camera.yaw -= motion.delta.x * camera.look_sensitivity

                # Update pitch (vertical rotation) with clamping
                camera.pitch -= motion.delta.y * camera.look_sensitivity
                camera.pitch = max(
                    -camera.max_pitch, min(camera.max_pitch, camera.pitch)
                )

                # Apply rotation to transform
                # Yaw around world Y axis, then pitch around local X axis
                yaw_quat = Quat.from_axis_angle(Vec3.Y, camera.yaw)
                pitch_quat = Quat.from_axis_angle(Vec3.X, camera.pitch)
                transform.rotation = yaw_quat * pitch_quat


def fly_camera_movement_system(
    query: Query[tuple[Mut[Transform], FlyCamera]],
    keyboard_input: Res[ButtonInput],
    camera_state: Res[FlyCameraState],
    time: Res[Time],
) -> None:
    """System that handles fly camera keyboard movement.

    Runs separately from mouse look to allow smooth movement even without mouse input.
    """
    for transform, camera in query:
        # Calculate movement speed with sprint modifier
        speed = camera.move_speed
        if camera_state.shift_pressed:
            speed *= camera.sprint_multiplier

        dt = time.delta_secs()
        movement_distance = speed * dt

        # Calculate direction vectors based on current rotation
        # Convert Dir3 to Vec3 for arithmetic
        forward = transform.forward().as_vec3()
        right = transform.right().as_vec3()
        up = Vec3.Y  # World up for vertical movement

        # Movement vector
        move_vec = Vec3(0.0, 0.0, 0.0)

        # Forward/Backward (W/S or Up/Down arrows)
        if keyboard_input.pressed(KeyCode.KeyW) or keyboard_input.pressed(
            KeyCode.ArrowUp
        ):
            move_vec = move_vec + forward
        if keyboard_input.pressed(KeyCode.KeyS) or keyboard_input.pressed(
            KeyCode.ArrowDown
        ):
            move_vec = move_vec - forward

        # Left/Right (A/D or Left/Right arrows)
        if keyboard_input.pressed(KeyCode.KeyA) or keyboard_input.pressed(
            KeyCode.ArrowLeft
        ):
            move_vec = move_vec - right
        if keyboard_input.pressed(KeyCode.KeyD) or keyboard_input.pressed(
            KeyCode.ArrowRight
        ):
            move_vec = move_vec + right

        # Up/Down (Q/E)
        if keyboard_input.pressed(KeyCode.KeyQ):
            move_vec = move_vec - up
        if keyboard_input.pressed(KeyCode.KeyE):
            move_vec = move_vec + up

        # Normalize and apply movement
        if move_vec.length() > 0.0:
            move_vec = move_vec.normalize()
            transform.translation = transform.translation + move_vec * movement_distance


@plugin
class FlyCameraPlugin(Plugin):
    """Plugin that adds fly camera controls to the app.

    Automatically registers systems for mouse look and keyboard movement.
    Add the FlyCamera component to a camera entity to enable controls.
    """

    def build(self, app: App) -> None:
        """Register fly camera systems and resources."""

        # Initialize camera state resource
        app.init_resource(FlyCameraState)

        # Add control systems to Update stage
        app.add_systems(Update, fly_camera_control_system)
        app.add_systems(Update, fly_camera_movement_system)
