# Foggy Scene Recipe

Moody fog scene: DistanceFog with dim lighting and scattered objects fading into the mist.

```python
import math
from pybevy.prelude import *
from pybevy.pbr import DistanceFog, FogFalloff

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
    # Camera with fog
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, 4.0, 12.0).looking_at(Vec3(0.0, 1.0, 0.0), Vec3.Y),
        DistanceFog(
            color=Color.srgb(0.5, 0.55, 0.6),
            falloff=FogFalloff.Exponential(0.04),
            directional_light_color=Color.srgb(1.0, 0.85, 0.6),
            directional_light_exponent=40.0,
        ),
        Name("camera"),
    )

    # Dim sun - partially obscured by fog
    commands.spawn(
        DirectionalLight(
            illuminance=4000.0,
            color=Color.srgb(1.0, 0.9, 0.75),
            shadow_maps_enabled=True,
        ),
        Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.6, 0.3, 0.0)),
        Name("sun"),
    )

    # Low ambient
    commands.insert_resource(GlobalAmbientLight(
        brightness=100.0,
        color=Color.srgb(0.6, 0.65, 0.75),
    ))

    # Ground
    ground_mesh = meshes.add(Plane3d(Vec3.Y, half_size=Vec2(30.0, 30.0)))
    ground_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.2, 0.25, 0.18),
    ))
    commands.spawn(Mesh3d(ground_mesh), MeshMaterial3d(ground_mat), Name("ground"))

    # Pillars receding into fog
    pillar_mesh = meshes.add(Cylinder(0.3, 3.0))
    pillar_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.4, 0.38, 0.35),
    ))
    for i in range(8):
        z = -i * 4.0
        for x in [-2.5, 2.5]:
            commands.spawn(
                Mesh3d(pillar_mesh), MeshMaterial3d(pillar_mat),
                Transform.from_xyz(x, 1.5, z),
            )

if __name__ == "__main__":
    main().run()
```

## Key points

- **DistanceFog** is a camera component - spawn it on the same entity as `Camera3d`
- **FogFalloff.Exponential(density)** - 0.002 subtle haze, 0.005 moderate, 0.04 thick
- **directional_light_color/exponent** creates god-ray effect through fog
- Lower sun **illuminance** (3000–5000) works better with fog than full 10000
- Fog density that looks fine in daylight may be invisible at night - test both
