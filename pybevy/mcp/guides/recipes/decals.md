# Decals Recipe

Project a texture onto existing geometry using clustered decals.

```python
from pybevy.prelude import *
from pybevy.light import ClusteredDecal

@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
    )

def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
    asset_server: Res[AssetServer],
) -> None:
    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(3, 3, 3).looking_at(Vec3.ZERO, Vec3.Y),
        Name("camera"),
    )
    commands.spawn(
        DirectionalLight(illuminance=10000.0, shadow_maps_enabled=True),
        Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.8, 0.4, 0.0)),
    )

    # Wall to project onto
    wall_mesh = meshes.add(Cuboid(4.0, 3.0, 0.2))
    wall_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.7, 0.65, 0.6),
    ))
    commands.spawn(
        Mesh3d(wall_mesh), MeshMaterial3d(wall_mat),
        Transform.from_xyz(0.0, 1.5, -1.0),
        Name("wall"),
    )

    # Decal projector
    commands.spawn(
        ClusteredDecal(
            base_color_texture=asset_server.load("bevy/textures/splat.png"),
        ),
        Transform.from_xyz(0.0, 1.5, -0.8),
        Name("decal"),
    )

if __name__ == "__main__":
    main().run()
```

## Key points

- **ClusteredDecal** projects a texture from the entity's position
- The `Transform` position + rotation controls where and how the decal is projected
- Use `tag` field to group decals (e.g., `tag=1` for blood, `tag=2` for bullet holes)
- The texture should have an alpha channel for the decal shape
