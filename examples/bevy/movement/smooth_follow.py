"""Demonstrates smooth interpolation to make one entity follow another.

This example shows a red sphere smoothly following a blue target sphere as it
moves to random positions. The following uses exponential decay interpolation
for smooth movement.

PyBevy Adaptations:
- No `Single` query type: Use regular Query with iteration
- No `.chain()` system ordering: Systems run in registration order
- Query disjointness: This example uses a resource to pass the target position
  to avoid having Query[Transform] and Query[Mut[Transform]] in the same system.
  Note: You CAN use Query[Mut[Transform], Without[MarkerA]] in separate systems
  to create disjoint queries, but not in the same system without ParamSet.
"""

import math
import random

from pybevy.ecs import Res, ResMut
from pybevy.prelude import *


# The sphere that the following sphere targets at all times
@component
class TargetSphere(Component):
    pass


# The speed of the target sphere moving to its next location
@resource
class TargetSphereSpeed(Resource):
    def __init__(self, speed: float = 5.0):
        self.speed = speed


# The position that the target sphere always moves linearly toward
@resource
class TargetPosition(Resource):
    def __init__(self, position: Vec3 = Vec3.ZERO):
        self.position = position


# Current position of the target sphere (updated each frame for follower)
@resource
class TargetCurrentPosition(Resource):
    def __init__(self, position: Vec3 = Vec3.ZERO):
        self.position = position


# The decay rate used by the smooth following
@resource
class DecayRate(Resource):
    def __init__(self, rate: float = 2.0):
        self.rate = rate


# The sphere that follows the target sphere by moving towards it with nudging
@component
class FollowingSphere(Component):
    pass


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # A plane
    commands.spawn(
        Mesh3d(meshes.add(Plane3d().mesh().size(12.0, 12.0))),
        MeshMaterial3d(materials.add(Color.srgb(0.3, 0.15, 0.3))),
        Transform.from_xyz(0.0, -2.5, 0.0),
    )

    # The target sphere (blue)
    commands.spawn(
        Mesh3d(meshes.add(Sphere(radius=0.3))),
        MeshMaterial3d(materials.add(Color.srgb(0.3, 0.15, 0.9))),
        TargetSphere(),
        Transform.from_xyz(0.0, 0.0, 0.0),
    )

    # The sphere that follows it (red)
    commands.spawn(
        Mesh3d(meshes.add(Sphere(radius=0.3))),
        MeshMaterial3d(materials.add(Color.srgb(0.9, 0.3, 0.3))),
        Transform.from_translation(Vec3(0.0, -2.0, 0.0)),
        FollowingSphere(),
    )

    # A light
    commands.spawn(
        PointLight(intensity=15_000_000.0, shadows_enabled=True),
        Transform.from_xyz(4.0, 8.0, 4.0),
    )

    # A camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-2.0, 3.0, 5.0).looking_at(Vec3.ZERO, Vec3.Y),
    )

    # Set starting values for resources used by the systems
    commands.insert_resource(TargetSphereSpeed(5.0))
    commands.insert_resource(DecayRate(2.0))
    commands.insert_resource(TargetPosition(Vec3.ZERO))
    commands.insert_resource(TargetCurrentPosition(Vec3.ZERO))


def move_target(
    target_query: Query[Mut[Transform], With[TargetSphere]],
    target_speed: Res[TargetSphereSpeed],
    target_pos: ResMut[TargetPosition],
    current_pos: ResMut[TargetCurrentPosition],
    time: Res[Time],
) -> None:
    # Get the single target transform
    for target in target_query:
        # Calculate direction to target position
        direction = target_pos.position - target.translation
        distance = direction.length()

        # If we're far enough from target, move toward it
        if distance > 0.001:  # Small threshold to avoid division by zero
            # Normalize direction
            dir_normalized = direction.normalize()

            # Calculate movement distance for this frame
            delta_time = time.delta_secs()

            # Avoid overshooting
            magnitude = min(distance, delta_time * target_speed.speed)
            target.translation += dir_normalized * magnitude
        else:
            # We've reached the target, generate a new random position
            # Sample a random point within a 4x4x4 cube
            target_pos.position = Vec3(
                random.uniform(-2.0, 2.0),
                random.uniform(-2.0, 2.0),
                random.uniform(-2.0, 2.0),
            )

        # Update current position resource for the follower
        current_pos.position = Vec3(target.translation.x, target.translation.y, target.translation.z)


def move_follower(
    following_query: Query[Mut[Transform], With[FollowingSphere]],
    target_current: Res[TargetCurrentPosition],
    decay_rate: Res[DecayRate],
    time: Res[Time],
) -> None:
    # Get the follower transform and move it
    for following in following_query:
        delta_time = time.delta_secs()

        # Implement smooth_nudge using exponential decay
        # This is equivalent to Bevy's smooth_nudge method
        # Formula: self += (target - self) * (1 - exp(-decay_rate * delta_time))
        decay_factor = 1.0 - math.exp(-decay_rate.rate * delta_time)
        following.translation += (
            target_current.position - following.translation
        ) * decay_factor


@entrypoint
def main(app: App) -> App:
    # Note: Systems run in the order they're added
    # move_target runs before move_follower (equivalent to .chain() in Rust)
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, move_target)
        .add_systems(Update, move_follower)
    )


if __name__ == "__main__":
    main().run()
