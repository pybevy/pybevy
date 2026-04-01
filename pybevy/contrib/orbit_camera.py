"""Orbit Camera Plugin - Mouse-controlled camera that orbits around a target point.

Usage:
    from pybevy.contrib import OrbitCameraPlugin

    app.add_plugins(OrbitCameraPlugin())
"""

import math
from dataclasses import dataclass

from ..app import App, Plugin, Update
from ..decorators import component, plugin, resource
from ..ecs import Component, MessageReader, Mut, Query, Res, ResMut, Resource
from ..input import (
    ButtonState,
    KeyboardInput,
    KeyCode,
    MouseButton,
    MouseInput,
    MouseMotion,
    MouseWheel,
)
from ..math import Vec3
from ..transform import Transform


@component(storage="python")
@dataclass
class OrbitCamera(Component):
    """Component for orbit camera controller.

    Attributes:
        distance: Distance from target point
        yaw: Horizontal rotation angle (radians)
        pitch: Vertical rotation angle (radians)
        target: Point to orbit around
        rotate_sensitivity: Mouse rotation sensitivity (default: 0.003)
        pan_sensitivity: Mouse pan sensitivity multiplier (default: 0.001)
        zoom_sensitivity: Mouse wheel zoom sensitivity (default: 0.1)
        min_distance: Minimum zoom distance (default: 5.0)
        max_distance: Maximum zoom distance (default: 1000.0)
    """

    distance: float
    yaw: float
    pitch: float
    target: "Vec3"
    rotate_sensitivity: float = 0.003
    pan_sensitivity: float = 0.001
    zoom_sensitivity: float = 0.1
    min_distance: float = 5.0
    max_distance: float = 1000.0


@resource
class OrbitCameraState(Resource):
    """Resource for tracking orbit camera state."""

    def __init__(self) -> None:
        self.shift_pressed = False


def orbit_camera_control_system(
    query: Query[tuple[Mut[Transform], Mut[OrbitCamera]]],
    mouse_buttons: Res[MouseInput],
    keyboard_input: MessageReader[KeyboardInput],
    mouse_motion: MessageReader[MouseMotion],
    mouse_wheel: MessageReader[MouseWheel],
    camera_state: ResMut[OrbitCameraState],
) -> None:
    """System that handles orbit camera mouse controls.

    Controls:
        - Left mouse drag: Rotate camera around target
        - Shift + Left mouse drag: Pan camera (move target)
        - Mouse wheel: Zoom in/out
    """
    # Track shift key state
    for event in keyboard_input:
        if event.key_code == KeyCode.ShiftLeft or event.key_code == KeyCode.ShiftRight:
            camera_state.shift_pressed = event.state == ButtonState.Pressed()

    for transform, camera in query:
        # Mouse drag (left button)
        if mouse_buttons.pressed(MouseButton.Left()):
            for motion in mouse_motion:
                # Check if Shift is held for panning
                if camera_state.shift_pressed:
                    # Pan camera by moving the target
                    # Calculate right and up vectors in camera space
                    forward = (camera.target - transform.translation).normalize()
                    right = forward.cross(Vec3.Y).normalize()
                    up = right.cross(forward).normalize()

                    # Pan speed scales with distance
                    pan_speed = camera.distance * camera.pan_sensitivity
                    camera.target = camera.target - right * motion.delta.x * pan_speed
                    camera.target = camera.target + up * motion.delta.y * pan_speed
                else:
                    # Rotate camera
                    camera.yaw -= motion.delta.x * camera.rotate_sensitivity
                    camera.pitch += motion.delta.y * camera.rotate_sensitivity

                    # Clamp pitch to avoid gimbal lock
                    camera.pitch = max(
                        -math.pi / 2 + 0.1, min(math.pi / 2 - 0.1, camera.pitch)
                    )

        # Mouse wheel to zoom
        for wheel in mouse_wheel:
            camera.distance -= wheel.y * camera.distance * camera.zoom_sensitivity
            # Clamp distance to reasonable values
            camera.distance = max(
                camera.min_distance, min(camera.max_distance, camera.distance)
            )

        # Update camera position based on orbit parameters
        # Calculate position using spherical coordinates
        x = camera.target.x + camera.distance * math.cos(camera.pitch) * math.sin(
            camera.yaw
        )
        y = camera.target.y + camera.distance * math.sin(camera.pitch)
        z = camera.target.z + camera.distance * math.cos(camera.pitch) * math.cos(
            camera.yaw
        )

        transform.translation = Vec3(x, y, z)
        # Look at target with proper up vector
        transform.looking_at(camera.target, Vec3.Y)


@plugin
class OrbitCameraPlugin(Plugin):
    """Plugin that adds orbit camera functionality.

    Automatically adds the orbit camera control system to Update stage.

    Features:
        - Orbit rotation with left mouse drag
        - Pan camera (move target) with Shift + left mouse drag
        - Zoom with mouse wheel
        - Configurable sensitivity and distance limits

    Example:
        >>> from pybevy.contrib import OrbitCameraPlugin
        >>> from pybevy.prelude import *
        >>>
        >>> app = App()
        >>> app.add_plugins(DefaultPlugins)
        >>> app.add_plugins(OrbitCameraPlugin())
        >>>
        >>> # Spawn a camera with orbit controls
        >>> def setup(commands: Commands) -> None:
        ...     commands.spawn(
        ...         Camera3d(),
        ...         Transform.from_xyz(0.0, 50.0, 100.0).looking_at(Vec3.ZERO, Vec3.Y),
        ...         OrbitCamera(
        ...             distance=100.0,
        ...             yaw=0.0,
        ...             pitch=0.4,
        ...             target=Vec3.ZERO,
        ...         ),
        ...     )
    """

    def build(self, app: "App") -> None:
        """Add orbit camera systems and resources to the app."""

        app.init_resource(OrbitCameraState)
        app.add_systems(Update, orbit_camera_control_system)
