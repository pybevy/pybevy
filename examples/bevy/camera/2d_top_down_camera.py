"""2D top-down camera with smooth player tracking.

Demonstrates:
- 2D camera following a player
- Smooth camera movement using exponential decay
- Keyboard input for player movement (WASD)
- Single query type for unique entities
- 2D mesh primitives (Circle, Rectangle)

Controls:
- W: Move up
- S: Move down
- A: Move left
- D: Move right
"""

import math

from pybevy import component
from pybevy.assets import Assets
from pybevy.ecs import Commands, Component, Mut, Res, ResMut, With
from pybevy.input import ButtonInput, KeyCode
from pybevy.math import Circle, Rectangle, Vec2, Vec3
from pybevy.mesh import Mesh2d, MeshMaterial2d
from pybevy.prelude import *
from pybevy.sprite import ColorMaterial

# Player movement speed factor
PLAYER_SPEED = 100.0

# How quickly should the camera snap to the desired location
CAMERA_DECAY_RATE = 2.0


@component
class Player(Component):
    """Marker component for the player entity."""


@resource
class PlayerPosition(Resource):
    """Resource to track player position (workaround for disjoint queries)."""

    def __init__(self):
        self.position = Vec3(0.0, 0.0, 0.0)


def setup_scene(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[ColorMaterial]],
) -> None:
    """Set up the game scene with world and player."""
    # World where we move the player
    world_mesh = Rectangle(1000.0, 700.0).mesh()
    world_material = ColorMaterial(color=Color.srgb(0.2, 0.2, 0.3))

    commands.spawn(
        Mesh2d(meshes.add(world_mesh)),
        MeshMaterial2d(materials.add(world_material)),
    )

    # Player - bright color for visual pop
    player_mesh = Circle(radius=25.0).mesh()
    player_material = ColorMaterial(color=Color.srgb(6.25, 9.4, 9.1))

    commands.spawn(
        Player(),
        Mesh2d(meshes.add(player_mesh)),
        MeshMaterial2d(materials.add(player_material)),
        Transform.from_xyz(0.0, 0.0, 2.0),
    )


def setup_camera(commands: Commands) -> None:
    """Set up the 2D camera."""
    commands.spawn(Camera2d())


def update_camera(
    camera_query: Query[Mut[Transform], With[Camera2d]],
    player_pos: Res[PlayerPosition],
    time: Res[Time],
) -> None:
    """Update camera position by smoothly tracking the player."""
    # Update camera to track player
    for camera in camera_query:
        # Target position (player x, y, but keep camera z)
        target = Vec3(player_pos.position.x, player_pos.position.y, camera.translation.z)

        # Smooth nudge using exponential decay
        # formula: position += (target - position) * (1 - exp(-decay_rate * delta_time))
        delta_time = time.delta_secs()
        decay_factor = 1.0 - math.exp(-CAMERA_DECAY_RATE * delta_time)

        diff_x = target.x - camera.translation.x
        diff_y = target.y - camera.translation.y
        diff_z = target.z - camera.translation.z

        camera.translation.x += diff_x * decay_factor
        camera.translation.y += diff_y * decay_factor
        camera.translation.z += diff_z * decay_factor


def move_player(
    player_query: Query[Mut[Transform], With[Player]],
    player_pos: ResMut[PlayerPosition],
    time: Res[Time],
    kb_input: Res[ButtonInput],
) -> None:
    """Update player position with keyboard inputs (WASD)."""
    for player in player_query:
        direction = Vec2(0.0, 0.0)

        if kb_input.pressed(KeyCode.KeyW):
            direction.y += 1.0

        if kb_input.pressed(KeyCode.KeyS):
            direction.y -= 1.0

        if kb_input.pressed(KeyCode.KeyA):
            direction.x -= 1.0

        if kb_input.pressed(KeyCode.KeyD):
            direction.x += 1.0

        # Normalize to prevent faster diagonal movement
        length = math.sqrt(direction.x * direction.x + direction.y * direction.y)
        if length > 0.0:
            direction.x /= length
            direction.y /= length

        # Update player position
        move_delta = direction.x * PLAYER_SPEED * time.delta_secs()
        player.translation.x += move_delta

        move_delta_y = direction.y * PLAYER_SPEED * time.delta_secs()
        player.translation.y += move_delta_y

        # Store player position in resource for camera to use
        player_pos.position = Vec3(player.translation.x, player.translation.y, 0.0)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(PlayerPosition())
        .add_systems(Startup, (setup_scene, setup_camera))
        .add_systems(Update, (move_player, update_camera))
    )


if __name__ == "__main__":
    main().run()
