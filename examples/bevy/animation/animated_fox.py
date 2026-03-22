"""Animated Fox — GLB skeletal animation with playback control.

Loads Fox.glb and plays the Run animation. Uses MessageReader[SceneInstanceReady]
to detect when the scene finishes loading, then finds the AnimationPlayer in the
scene hierarchy and starts playback.
"""
from pybevy.animation import (
    AnimationClip,
    AnimationGraph,
    AnimationGraphHandle,
    AnimationNodeIndex,
    AnimationPlayer,
)
from pybevy.gltf import GltfAssetLabel
from pybevy.prelude import *
from pybevy.scene import Scene, SceneInstanceReady, SceneRoot

FOX_PATH = "bevy/models/animated/Fox.glb"


@component
class AnimationToPlay(Component):
    def __init__(
        self,
        graph_handle: Handle[AnimationGraph],
        index: AnimationNodeIndex,
    ) -> None:
        self.graph_handle = graph_handle
        self.index = index


def setup(
    commands: Commands,
    asset_server: Res[AssetServer],
    graphs: ResMut[Assets[AnimationGraph]],
) -> None:
    # Animation graph — Run animation (index 2)
    graph, index = AnimationGraph.from_clip(
        asset_server.load(
            GltfAssetLabel.Animation(2).from_asset(FOX_PATH), AnimationClip
        ),
    )
    graph_handle = graphs.add(graph)

    # Fox with animation info
    commands.spawn(
        AnimationToPlay(graph_handle, index),
        SceneRoot(asset_server.load(
            GltfAssetLabel.Scene(0).from_asset(FOX_PATH), Scene
        )),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(100.0, 100.0, 150.0).looking_at(Vec3(0.0, 20.0, 0.0), Vec3.Y),
    )

    # Light
    commands.spawn(
        DirectionalLight(illuminance=5000.0, shadows_enabled=True),
        Transform.from_rotation(Quat.from_euler(EulerRot.ZYX, 0.0, 1.0, -0.785)),
    )
    commands.insert_resource(GlobalAmbientLight(brightness=2000.0))


def play_animation_when_ready(
    commands: Commands,
    scene_ready: MessageReader[SceneInstanceReady],
    animations_to_play: Query[AnimationToPlay],
    children_query: Query[Children],
    players: Query[Mut[AnimationPlayer]],
) -> None:
    for event in scene_ready:
        anim = animations_to_play.get(event.entity)
        if anim is None:
            continue

        for child in iter_descendants(event.entity, children_query):
            player = players.get(child)
            if player is not None:
                player.play(anim.index).repeat()
                commands.entity(child).insert(
                    AnimationGraphHandle(anim.graph_handle)
                )


def iter_descendants(entity: Entity, children_query: Query[Children]):
    children_list = children_query.get(entity)
    if children_list is None:
        return
    for child in children_list:
        yield child
        yield from iter_descendants(child, children_query)


@entrypoint
def main(app: App) -> App:
    app.add_plugins(DefaultPlugins)
    app.add_systems(Startup, setup)
    app.add_systems(Update, play_animation_when_ready)
    return app


if __name__ == "__main__":
    main().run()
