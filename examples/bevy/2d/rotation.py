"""
Demonstrates rotating entities in 2D using quaternions.

Based on Bevy's examples/2d/rotation.rs, this example shows:
- Using FixedUpdate schedule for consistent physics
- Using Single<T> to access the player entity
- Combining Single with Query for disjoint entity sets
- 2D rotation and movement using quaternions

NOTE: The snap_to_player_system currently fails due to a PyBevy limitation.
PyBevy doesn't yet recognize disjoint filters (With/Without) as making
queries safe for simultaneous mutable and immutable access. This works
in Rust Bevy but not in PyBevy yet.
"""

import math
from dataclasses import dataclass

from pybevy import component
from pybevy.app import App
from pybevy.assets import AssetServer
from pybevy.ecs import Commands, Component, Mut, Query, Single, With, Without
from pybevy.input import ButtonInput, KeyCode
from pybevy.math import Quat, Vec2, Vec3
from pybevy.prelude import (
    Camera2d,
    DefaultPlugins,
    FixedUpdate,
    Res,
    Sprite,
    Startup,
    Time,
    Transform,
    entrypoint,
)

BOUNDS = Vec2(1200.0, 640.0)


@component
@dataclass
class Player(Component):
    """Player component with movement and rotation speeds."""

    movement_speed: float = 0.0  # Meters per second
    rotation_speed: float = 0.0  # Radians per second


@component
class SnapToPlayer(Component):
    """Marker component for entities that snap to face the player."""



@component
@dataclass
class RotateToPlayer(Component):
    """Component for entities that smoothly rotate to face the player."""

    rotation_speed: float = 0.0  # Radians per second


def setup(commands: Commands, assets: AssetServer) -> None:
    """Set up the game entities and camera."""
    ship_handle = assets.load_image("bevy/textures/simplespace/ship_C.png")
    enemy_a_handle = assets.load_image("bevy/textures/simplespace/enemy_A.png")
    enemy_b_handle = assets.load_image("bevy/textures/simplespace/enemy_B.png")

    # Spawn camera
    commands.spawn(Camera2d())

    horizontal_margin = BOUNDS.x / 4.0
    vertical_margin = BOUNDS.y / 4.0

    # Player controlled ship
    player = Player()
    player.movement_speed = 500.0  # Meters per second
    player.rotation_speed = 3.14159 * 2.0  # ~360 degrees per second

    commands.spawn(Sprite.from_image(ship_handle), player)

    # Enemy that snaps to face the player (spawns on bottom and left)
    commands.spawn(
        Sprite.from_image(enemy_a_handle),
        Transform.from_xyz(0.0 - horizontal_margin, 0.0, 0.0),
        SnapToPlayer(),
    )
    commands.spawn(
        Sprite.from_image(enemy_a_handle),
        Transform.from_xyz(0.0, 0.0 - vertical_margin, 0.0),
        SnapToPlayer(),
    )

    # Enemy that rotates to face the player (spawns on top and right)
    rotate_enemy1 = RotateToPlayer()
    rotate_enemy1.rotation_speed = 0.785  # ~45 degrees per second

    commands.spawn(
        Sprite.from_image(enemy_b_handle),
        Transform.from_xyz(0.0 + horizontal_margin, 0.0, 0.0),
        rotate_enemy1,
    )

    rotate_enemy2 = RotateToPlayer()
    rotate_enemy2.rotation_speed = 1.571  # ~90 degrees per second

    commands.spawn(
        Sprite.from_image(enemy_b_handle),
        Transform.from_xyz(0.0, 0.0 + vertical_margin, 0.0),
        rotate_enemy2,
    )


def player_movement_system(
    time: Res[Time],
    keyboard_input: Res[ButtonInput],
    player_query: Single[tuple[Player, Mut[Transform]]],
) -> None:
    """Handle player movement and rotation based on keyboard input.

    Runs in FixedUpdate for consistent physics regardless of frame rate.
    Uses Single<T> to access exactly one player entity.
    """
    for ship, transform in player_query:
        rotation_factor = 0.0
        movement_factor = 0.0

        if keyboard_input.pressed(KeyCode.ArrowLeft):
            rotation_factor += 1.0

        if keyboard_input.pressed(KeyCode.ArrowRight):
            rotation_factor -= 1.0

        if keyboard_input.pressed(KeyCode.ArrowUp):
            movement_factor += 1.0

        # Update rotation around Z axis (perpendicular to 2D plane)
        transform.rotate_z(rotation_factor * ship.rotation_speed * time.delta_secs())

        # Get forward vector by applying current rotation to initial facing direction
        movement_direction = transform.rotation * Vec3.Y
        movement_distance = movement_factor * ship.movement_speed * time.delta_secs()
        translation_delta = movement_direction * movement_distance
        transform.translation += translation_delta

        # Bound the ship within invisible level bounds
        half_bounds = BOUNDS / 2.0
        extents = Vec3(half_bounds.x, half_bounds.y, 0.0)
        transform.translation = transform.translation.min(extents).max(-extents)


def snap_to_player_system(
    player_transform: Single[Transform, With[Player]],
    query: Query[Mut[Transform], tuple[With[SnapToPlayer], Without[Player]]],
) -> None:
    """Snap enemies to immediately face the player.

    Demonstrates using With/Without filters to create disjoint queries.
    """
    for player_t in player_transform:
        player_translation = player_t.translation.xy()

        for enemy_transform in query:
            enemy_pos = enemy_transform.translation.xy()
            to_player_vec = player_translation - enemy_pos
            to_player = to_player_vec.normalize()

            to_player_3d = Vec3(to_player.x, to_player.y, 0.0)
            rotate_to_player = Quat.from_rotation_arc(Vec3.Y, to_player_3d)

            enemy_transform.rotation = rotate_to_player


def rotate_to_player_system(
    time: Res[Time],
    query: Query[tuple[RotateToPlayer, Mut[Transform]], Without[Player]],
    player_transform: Single[Transform, With[Player]],
) -> None:
    """Smoothly rotate enemies to face the player.

    Uses dot product to determine rotation direction and limit rotation speed.
    """
    for player_t in player_transform:
        player_translation = player_t.translation.xy()

        for config, enemy_transform in query:
            enemy_forward_3d = enemy_transform.rotation * Vec3.Y
            enemy_forward = Vec2(enemy_forward_3d.x, enemy_forward_3d.y)

            enemy_pos = enemy_transform.translation.xy()
            to_player_vec = player_translation - enemy_pos
            to_player = to_player_vec.normalize()

            forward_dot_player = enemy_forward.dot(to_player)

            # If dot product is ~1.0, enemy already faces player
            if abs(forward_dot_player - 1.0) < 0.0001:
                continue

            enemy_right_3d = enemy_transform.rotation * Vec3.X
            enemy_right = Vec2(enemy_right_3d.x, enemy_right_3d.y)

            right_dot_player = enemy_right.dot(to_player)

            # Determine rotation direction
            rotation_sign = -1.0 if right_dot_player >= 0.0 else 1.0

            # Limit rotation to avoid overshooting
            max_angle = math.acos(max(-1.0, min(1.0, forward_dot_player)))

            rotation_angle = rotation_sign * min(
                config.rotation_speed * time.delta_secs(), max_angle
            )

            enemy_transform.rotate_z(rotation_angle)


@entrypoint
def main(app: App) -> App:
    """
    Entry point demonstrating FixedUpdate schedule and Single<T> queries.

    Controls:
        - Arrow Up: Move Forward
        - Arrow Left/Right: Turn
    """
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(
            FixedUpdate,
            (
                player_movement_system,
                snap_to_player_system,
                rotate_to_player_system,
            ),
        )
    )


if __name__ == "__main__":
    main().run()
