"""Illustrates various ways to load assets.

Demonstrates:
- Loading assets from files with AssetServer
- Loading GLTF sub-assets
- Checking if assets are loaded
- Adding assets directly to Assets<T> storage
- Asset handles and dependencies
"""

from pybevy.ecs import Res, ResMut
from pybevy.prelude import *


def setup(
    commands: Commands,
    asset_server: Res[AssetServer],
    meshes: Res[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Load various assets and spawn entities."""
    # By default AssetServer loads assets from the "assets" folder
    # For example, this loads: assets/models/cube/cube.gltf
    cube_handle = asset_server.load_mesh(
        GltfAssetLabel.Primitive(mesh=0, primitive=0).from_asset("bevy/models/cube/cube.gltf")
    )
    sphere_handle = asset_server.load_mesh(
        GltfAssetLabel.Primitive(mesh=0, primitive=0).from_asset(
            "bevy/models/sphere/sphere.gltf"
        )
    )

    # All assets end up in their Assets<T> collection once loaded
    # Note: Assets load asynchronously, so they may not be available immediately
    sphere_mesh = meshes.get(sphere_handle)
    if sphere_mesh is not None:
        # This probably won't run immediately because assets load in parallel
        print(f"Sphere topology: {sphere_mesh.primitive_topology()}")
    else:
        print("Sphere hasn't loaded yet")

    # Note: PyBevy doesn't have LoadedFolder yet, but you can load individual assets
    # from a folder by calling load() for each file

    # Load a specific asset from the torus folder
    torus_handle = asset_server.load_mesh(
        GltfAssetLabel.Primitive(mesh=0, primitive=0).from_asset(
            "bevy/models/torus/torus.gltf"
        )
    )

    # You can also add assets directly to their Assets<T> storage
    material_handle = materials.add(
        StandardMaterial(base_color=Color.srgb(0.8, 0.7, 0.6))
    )

    # Spawn entities with the loaded assets
    # Torus
    commands.spawn(
        Mesh3d(torus_handle),
        MeshMaterial3d(material_handle),
        Transform.from_xyz(-3.0, 0.0, 0.0),
    )

    # Cube
    commands.spawn(
        Mesh3d(cube_handle),
        MeshMaterial3d(material_handle),
        Transform.from_xyz(0.0, 0.0, 0.0),
    )

    # Sphere
    commands.spawn(
        Mesh3d(sphere_handle),
        MeshMaterial3d(material_handle),
        Transform.from_xyz(3.0, 0.0, 0.0),
    )

    # Light
    commands.spawn(
        PointLight(intensity=15_000_000.0, shadows_enabled=True),
        Transform.from_xyz(4.0, 8.0, 4.0),
    )

    # Camera
    commands.spawn(
        Camera3d(), Transform.from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3.ZERO, Vec3.Y)
    )


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)


if __name__ == "__main__":
    main().run()
