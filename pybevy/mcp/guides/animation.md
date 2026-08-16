# Animation Guide

Playing animations from GLB/GLTF models and controlling playback at runtime.

## Loading and Playing a GLB Animation

Full pipeline: load clip → build graph → spawn with player → play in system.

```python
from pybevy.prelude import *
from pybevy.animation import (
    AnimationClip, AnimationGraph, AnimationGraphHandle,
    AnimationNodeIndex, AnimationPlayer, AnimationTransitions,
)
from pybevy.world_serialization import WorldAsset, WorldAssetRoot, WorldInstanceReady
from pybevy.gltf import GltfAssetLabel

@component
class AnimationToPlay(Component):
    """Stores animation info to apply when the scene is ready."""
    def __init__(self, graph_handle: Handle[AnimationGraph], index: AnimationNodeIndex):
        self.graph_handle = graph_handle
        self.index = index

def setup(
    commands: Commands,
    asset_server: Res[AssetServer],
    graphs: ResMut[Assets[AnimationGraph]],
) -> None:
    # Load animation clip - #Animation0, #Animation1, etc.
    clip = asset_server.load(
        GltfAssetLabel.Animation(0).from_asset("models/fox.glb"),
        AnimationClip,
    )

    # Build graph - returns (graph, node_index) tuple
    graph, index = AnimationGraph.from_clip(clip)
    graph_handle = graphs.add(graph)

    # Spawn model with animation components
    commands.spawn(
        WorldAssetRoot(asset_server.load(
            GltfAssetLabel.Scene(0).from_asset("models/fox.glb"), WorldAsset
        )),
        Transform.from_xyz(0.0, 0.0, 0.0),
        Name("fox"),
        AnimationToPlay(graph_handle, index),
    )
```

## Starting Playback After Scene Loads

GLB scenes load asynchronously. The `AnimationPlayer` component is created by Bevy inside the scene hierarchy, not on your spawned entity. Use `WorldInstanceReady` to detect when loading finishes, then find the player in children.

```python
def play_when_ready(
    commands: Commands,
    scene_ready: MessageReader[WorldInstanceReady],
    animations: Query[AnimationToPlay],
    children_query: Query[Children],
    players: Query[Mut[AnimationPlayer]],
) -> None:
    for event in scene_ready:
        anim = animations.get(event.entity)
        if anim is None:
            continue

        # Walk the hierarchy to find the AnimationPlayer
        for child in iter_descendants(event.entity, children_query):
            player = players.get(child)
            if player is not None:
                player.play(anim.index).repeat()
                commands.entity(child).insert(
                    AnimationGraphHandle(anim.graph_handle)
                )

def iter_descendants(entity: Entity, children_query: Query[Children]):
    """Recursively yield all descendants of an entity."""
    children_list = children_query.get(entity)
    if children_list is None:
        return
    for child in children_list:
        yield child
        yield from iter_descendants(child, children_query)
```

**Critical gotcha:** `AnimationGraphHandle` must be inserted on the **same entity** as the `AnimationPlayer` (a child inside the scene hierarchy), not on the `WorldAssetRoot` entity. If placed on the wrong entity, nothing happens and there's no error.

## Animation Clip Naming

GLB files use index-based naming:
- `GltfAssetLabel.Animation(0)` → first animation
- `GltfAssetLabel.Animation(1)` → second animation
- `GltfAssetLabel.Animation(2)` → third animation

Load the root `Gltf` asset to discover its clips. `gltf.animations` lists every
`Handle[AnimationClip]`; `gltf.named_animations` maps source names to handles
for clips that have names in the source file. Use the index-based labels when
the file has unnamed clips.

## ActiveAnimation API

`player.play(index)` returns an `ActiveAnimation` with chainable methods:

```python
anim = player.play(index)
anim.repeat()                    # Loop forever
anim.set_speed(2.0)             # Double speed
anim.set_weight(0.5)            # Blend weight

# Other methods
anim.pause()                     # Pause playback
anim.resume()                    # Resume paused animation
anim.seek_to(1.5)               # Jump to 1.5 seconds
anim.rewind()                   # Back to start
anim.is_finished                # Check if done (property)
anim.elapsed                    # Current time (property)
```

## Repeat Modes

```python
from pybevy.animation import RepeatAnimation

anim.set_repeat(RepeatAnimation.Forever())   # Loop indefinitely (same as .repeat())
anim.set_repeat(RepeatAnimation.Count(3))    # Play 3 times then stop
anim.set_repeat(RepeatAnimation.Never())     # Play once (default)
```

## Smooth Transitions Between Animations

Use `AnimationTransitions` to cross-fade between animations (e.g., idle → walk → run):

```python
# Spawn with AnimationTransitions alongside AnimationPlayer
commands.entity(player_entity).insert(AnimationTransitions())

# In a system - fade to new animation over 0.3 seconds
def switch_animation(
    query: Query[tuple[Mut[AnimationPlayer], Mut[AnimationTransitions]]],
) -> None:
    for player, transitions in query:
        transitions.play(player, run_index, 0.3)  # 0.3s cross-fade
```

**Important:** When using `AnimationTransitions`, always play animations through `transitions.play()`, not `player.play()`. Direct player manipulation will confuse the transition system.

## Multiple Clips from One Model

Load multiple clips and build a graph with all of them:

```python
clips = [
    asset_server.load(GltfAssetLabel.Animation(i).from_asset("model.glb"), AnimationClip)
    for i in range(3)  # 3 animations
]
graph, _root_index = AnimationGraph.from_clips(clips)
```

`from_clips` returns `(graph, root_index)`. Individual clip indices can be retrieved from the graph's nodes.

## AnimationPlayer Methods

| Method | Description |
|--------|-------------|
| `play(index)` | Start/resume animation, returns `ActiveAnimation` |
| `start(index)` | Start from beginning (resets if already playing) |
| `stop(index)` | Stop specific animation |
| `stop_all()` | Stop all animations |
| `pause_all()` / `resume_all()` | Pause/resume all |
| `adjust_speeds(factor)` | Multiply all active animation speeds |
| `all_finished` | True when all animations complete (property) |
| `is_playing_animation(index)` | Check if specific animation is active |
| `animation(index)` | Get a read-only `ActiveAnimation` reference (None if not playing) |
| `animation_mut(index)` | Get a mutable `ActiveAnimation` reference (None if not playing) |

## See Also

- `guide://3d-models` - Loading GLB models, scale, origin gotchas
- `guide://patterns` - System parameter patterns, hierarchy traversal
