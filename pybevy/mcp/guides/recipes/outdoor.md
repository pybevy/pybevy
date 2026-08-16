# Outdoor Scene Recipe

Complete starter scene: bright sun, ambient fill, ground plane, colored cubes, and a camera with bloom.

```python
import math
from pybevy.prelude import *

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
) -> None:
    # Camera with bloom and fog
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(8.0, 6.0, 8.0).looking_at(Vec3.ZERO, Vec3.Y),
        Bloom(intensity=0.15, low_frequency_boost=0.5),
        DistanceFog(
            color=Color.srgba(0.8, 0.85, 0.9, 1.0),
            falloff=FogFalloff.Linear(start=8.0, end=30.0),
        ),
        Name("camera"),
    )

    # Sun
    commands.spawn(
        DirectionalLight(
            illuminance=10000.0,
            color=Color.srgb(1.0, 0.95, 0.85),
            shadow_maps_enabled=True,
        ),
        Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.8, 0.4, 0.0)),
        Name("sun"),
    )

    # Ambient fill
    commands.insert_resource(GlobalAmbientLight(brightness=300.0))

    # Ground plane
    ground_mesh = meshes.add(Plane3d(Vec3.Y, half_size=Vec2(10.0, 10.0)))
    ground_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.3, 0.5, 0.2),
    ))
    commands.spawn(Mesh3d(ground_mesh), MeshMaterial3d(ground_mat), Name("ground"))

    # Scatter some cubes
    cube_mesh = meshes.add(Cuboid(0.6, 0.6, 0.6))
    colors = [
        Color.srgb(0.8, 0.2, 0.2),
        Color.srgb(0.2, 0.6, 0.8),
        Color.srgb(0.9, 0.7, 0.1),
        Color.srgb(0.4, 0.8, 0.3),
    ]
    for i, color in enumerate(colors):
        angle = i * math.pi * 0.5
        x = math.cos(angle) * 3.0
        z = math.sin(angle) * 3.0
        mat = materials.add(StandardMaterial(base_color=color))
        commands.spawn(
            Mesh3d(cube_mesh),
            MeshMaterial3d(mat),
            Transform.from_xyz(x, 0.3, z),
        )

if __name__ == "__main__":
    main().run()
```

## Key points

- **Sun direction** comes from `Transform` rotation, not translation
- **GlobalAmbientLight** brightness 200–400 prevents pitch-black shadows
- **Bloom** adds the required HDR marker to its camera and works best with emissive materials
- **DistanceFog** with `FogFalloff.Linear(start, end)` - required for atmospheric depth
- Ground uses `Plane3d(Vec3.Y, half_size=...)` - the normal vector comes first
