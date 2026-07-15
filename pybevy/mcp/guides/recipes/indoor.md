# Indoor Scene Recipe

Torch-lit interior: low ambient, warm point lights with shadows, and simple room geometry.

```python
from pybevy.prelude import *
from pybevy.color import LinearRgba

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
    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, 3.0, 8.0).looking_at(Vec3(0.0, 1.5, 0.0), Vec3.Y),
        Bloom(intensity=0.3, low_frequency_boost=0.7),
        Name("camera"),
    )

    # Very dim ambient - torches provide most light
    commands.insert_resource(GlobalAmbientLight(brightness=30.0))

    # Floor
    floor_mesh = meshes.add(Plane3d(Vec3.Y, half_size=Vec2(5.0, 5.0)))
    stone_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.25, 0.22, 0.2),
    ))
    commands.spawn(Mesh3d(floor_mesh), MeshMaterial3d(stone_mat), Name("floor"))

    # Back wall
    wall_mesh = meshes.add(Cuboid(10.0, 4.0, 0.2))
    wall_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.3, 0.28, 0.25),
    ))
    commands.spawn(
        Mesh3d(wall_mesh), MeshMaterial3d(wall_mat),
        Transform.from_xyz(0.0, 2.0, -5.0),
        Name("wall"),
    )

    # Two torch lights (warm point lights)
    for i, x in enumerate([-3.0, 3.0]):
        commands.spawn(
            PointLight(
                intensity=80000.0,
                color=Color.srgb(1.0, 0.7, 0.3),
                range=10.0,
                shadow_maps_enabled=True,
            ),
            Transform.from_xyz(x, 2.5, -4.0),
            Name(f"torch_{i}"),
        )

        # Emissive torch marker
        torch_mesh = meshes.add(Sphere(0.08))
        torch_mat = materials.add(StandardMaterial(
            base_color=Color.srgb(1.0, 0.6, 0.1),
            emissive=LinearRgba.rgb(12.0, 6.0, 1.0),
            unlit=True,
        ))
        commands.spawn(
            Mesh3d(torch_mesh), MeshMaterial3d(torch_mat),
            Transform.from_xyz(x, 2.5, -4.0),
        )

    # A crate in the middle
    crate_mesh = meshes.add(Cuboid(1.0, 1.0, 1.0))
    crate_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.5, 0.35, 0.15),
    ))
    commands.spawn(
        Mesh3d(crate_mesh), MeshMaterial3d(crate_mat),
        Transform.from_xyz(0.0, 0.5, 0.0),
        Name("crate"),
    )

if __name__ == "__main__":
    main().run()
```

## Key points

- **Low ambient** (20–50) keeps areas outside light range dark
- **PointLight range** controls falloff distance - 8–12 for torches
- **Emissive + unlit** spheres make visible light sources that glow with bloom
- **Shadows** are critical indoors - enable on at least one light
