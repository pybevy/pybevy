"""Skinned mesh animation using GLTF model with hierarchy traversal.

Demonstrates:
- Loading GLTF scenes with WorldAssetRoot
- Traversing entity hierarchies with ChildOf and Children
- Animating skeletal joints by navigating parent-child relationships
- Time-based rotation animation

This example loads a simple skinned mesh from a GLTF file and animates
one of its joints by traversing the entity hierarchy.

Scene hierarchy:
```
<Parent entity>
  + Mesh node (without Mesh3d or SkinnedMesh component)
    + Skinned mesh entity (with Mesh3d and SkinnedMesh component)
    + First joint
      + Second joint  <- We animate this
```
"""

import math

from pybevy.mesh import SkinnedMesh
from pybevy.prelude import *


def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    """Set up camera and load GLTF scene."""
    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3(0.0, 1.0, 0.0), Vec3.Y),
    )

    # Spawn the first scene from the SimpleSkin GLTF file
    # Note: Ensure models/SimpleSkin/SimpleSkin.gltf exists in your assets directory
    scene_label = GltfAssetLabel.Scene(0)
    asset_path = scene_label.from_asset("bevy/models/SimpleSkin/SimpleSkin.gltf")
    scene_handle = asset_server.load(asset_path, WorldAsset)
    commands.spawn(WorldAssetRoot(scene_handle))


def joint_animation(
    time: Res[Time],
    skinned_meshes: Query[ChildOf, With[SkinnedMesh]],
    parents: Query[Children],
    transforms: Query[Mut[Transform]],
) -> None:
    """Animate the second joint in the skeletal hierarchy.

    This demonstrates:
    - Using ChildOf to find parent entities
    - Using Children to traverse down the hierarchy
    - Indexing Children to access specific child entities
    """
    # Iterate over all skinned mesh entities
    for child_of in skinned_meshes:
        if child_of is None:
            continue
        # The skinned mesh's parent is the mesh node
        mesh_node_entity = child_of.parent()

        # Get the children of the mesh node
        mesh_node_children = parents.get(mesh_node_entity)
        if mesh_node_children is None:
            continue

        # The first joint is the second child of the mesh node (index 1)
        if len(mesh_node_children) < 2:
            continue
        first_joint_entity = mesh_node_children[1]

        # Get the children of the first joint
        first_joint_children = parents.get(first_joint_entity)
        if first_joint_children is None or len(first_joint_children) == 0:
            continue

        # The second joint is the first child of the first joint
        second_joint_entity = first_joint_children[0]

        # Get the transform of the second joint and animate it
        transform = transforms.get(second_joint_entity)
        if transform is not None:
            # Animate rotation using a sine wave
            angle = (math.pi / 2.0) * math.sin(time.elapsed_secs())
            transform.rotation = Quat.from_rotation_z(angle)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(GlobalAmbientLight(brightness=750.0))
        .add_systems(Startup, setup)
        .add_systems(Update, joint_animation)
    )


if __name__ == "__main__":
    main().run()
