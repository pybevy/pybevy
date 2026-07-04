"""Play an animation with morph targets.

Demonstrates:
- Loading GLTF scenes with morph target animations
- WorldInstanceReady message for scene load notifications
- MessageReader pattern for scene loading events
- AnimationGraph.from_clip() for animation setup
- Recursive hierarchy traversal with Children component
- AnimationPlayer setup after scene is loaded

This example loads a GLTF model with morph target animation and plays it in a loop.

Note: Uses MessageReader[WorldInstanceReady] instead of Bevy's Observer pattern,
which provides equivalent functionality for scene loading notifications.
"""

import math

from pybevy.animation import (
    AnimationClip,
    AnimationGraph,
    AnimationGraphHandle,
    AnimationNodeIndex,
    AnimationPlayer,
)
from pybevy.prelude import *
from pybevy.world_serialization import WorldInstanceReady


@component
class AnimationToPlay(Component):
    """Stores animation graph and node index to apply when scene is ready."""

    def __init__(self, graph_handle: Handle[AnimationGraph], index: AnimationNodeIndex):
        self.graph_handle = graph_handle
        self.index = index


def setup(
    commands: Commands,
    asset_server: Res[AssetServer],
    graphs: ResMut[Assets[AnimationGraph]],
) -> None:
    """Set up the scene with GLTF model and animation."""
    gltf_path = "bevy/models/animated/MorphStressTest.gltf"

    # Create animation graph from the GLTF animation clip
    animation_clip = asset_server.load(
        GltfAssetLabel.Animation(2).from_asset(gltf_path), AnimationClip
    )
    graph, index = AnimationGraph.from_clip(animation_clip)

    # Store graph handle for later use
    graph_handle = graphs.add(graph)

    # Spawn scene root with animation info
    commands.spawn(
        AnimationToPlay(graph_handle, index),
        WorldAssetRoot(
            asset_server.load(GltfAssetLabel.Scene(0).from_asset(gltf_path), WorldAsset)
        ),
    )

    # Directional light
    commands.spawn(
        DirectionalLight(),
        Transform.from_rotation(Quat.from_euler(EulerRot.ZYX, 0.0, 0.0, math.pi / 2.0)),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(3.0, 2.1, 10.2).looking_at(Vec3.ZERO, Vec3.Y),
    )


def play_animation_when_ready(
    commands: Commands,
    scene_ready: MessageReader[WorldInstanceReady],
    animations_to_play: Query[AnimationToPlay],
    children: Query[Children],
    players: Query[Mut[AnimationPlayer]],
) -> None:
    """Play animation when scene instance is ready.

    This uses MessageReader instead of Bevy's Observer pattern.
    The WorldInstanceReady message is sent when scenes finish loading.
    """
    for event in scene_ready:
        # Get the animation info for this scene entity
        animation_to_play = animations_to_play.get(event.entity)
        if animation_to_play is None:
            continue

        # Find AnimationPlayer in the scene hierarchy
        for child in iter_descendants(event.entity, children):
            player = players.get(child)
            if player is not None:
                # Start playing the animation with repeat
                player.play(animation_to_play.index).repeat()

                # Insert animation graph handle to connect player to mesh
                commands.entity(child).insert(
                    AnimationGraphHandle(animation_to_play.graph_handle)
                )


def iter_descendants(entity: Entity, children_query: Query[Children]):
    """Recursively iterate over all descendants of an entity.

    Equivalent to Bevy's children.iter_descendants().
    """
    children_list = children_query.get(entity)
    if children_list is None:
        return

    for child in children_list:
        yield child
        yield from iter_descendants(child, children_query)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(
            GlobalAmbientLight(
                color=Color.WHITE,
                brightness=150.0,
            )
        )
        .add_systems(Startup, setup)
        .add_systems(Update, play_animation_when_ready)
    )


if __name__ == "__main__":
    main().run()
