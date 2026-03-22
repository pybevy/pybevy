# Glass Scene Recipe

Scene with glass sphere, frosted panel, and backlit leaf demonstrating transmission materials.

```python
import math
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
    # Camera — bloom helps glass reflections pop
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(6, 4, 6).looking_at(Vec3(0, 1, 0), Vec3.Y),
        Bloom(intensity=0.15, low_frequency_boost=0.5),
        Exposure.INDOOR,
        Name("camera"),
    )

    # Bright directional light (needed for transmission to be visible)
    commands.spawn(
        DirectionalLight(illuminance=15000.0, shadows_enabled=True),
        Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.7, 0.5, 0.0)),
        Name("sun"),
    )
    commands.insert_resource(GlobalAmbientLight(brightness=200.0))

    # Ground
    ground_mesh = meshes.add(Plane3d(Vec3.Y, half_size=Vec2(8.0, 8.0)))
    ground_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.6, 0.55, 0.5),
        perceptual_roughness=0.8,
    ))
    commands.spawn(Mesh3d(ground_mesh), MeshMaterial3d(ground_mat), Name("ground"))

    # --- Glass Sphere ---
    glass_mat = materials.add(StandardMaterial(
        base_color=Color.srgba(0.9, 0.95, 1.0, 0.05),
        specular_transmission=1.0,
        ior=1.5,
        thickness=1.5,
        perceptual_roughness=0.0,
        metallic=0.0,
        reflectance=0.5,
        alpha_mode=AlphaMode.Blend(),
    ))
    sphere_mesh = meshes.add(Sphere(0.8).mesh().ico(5))
    commands.spawn(
        Mesh3d(sphere_mesh), MeshMaterial3d(glass_mat),
        Transform.from_xyz(0.0, 0.8, 0.0),
        Name("glass_sphere"),
    )

    # --- Frosted Panel ---
    frosted_mat = materials.add(StandardMaterial(
        base_color=Color.srgba(0.85, 0.9, 0.95, 0.1),
        specular_transmission=0.8,
        ior=1.5,
        thickness=0.3,
        perceptual_roughness=0.4,
        alpha_mode=AlphaMode.Blend(),
    ))
    panel_mesh = meshes.add(Cuboid(2.0, 1.5, 0.05))
    commands.spawn(
        Mesh3d(panel_mesh), MeshMaterial3d(frosted_mat),
        Transform.from_xyz(-2.5, 0.75, 0.0),
        Name("frosted_panel"),
    )

    # --- Backlit Leaf ---
    leaf_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.15, 0.45, 0.08),
        diffuse_transmission=0.5,
        double_sided=True,
        cull_mode=None,
        perceptual_roughness=0.8,
        metallic=0.0,
    ))
    leaf_mesh = meshes.add(Cuboid(1.2, 0.8, 0.01))
    commands.spawn(
        Mesh3d(leaf_mesh), MeshMaterial3d(leaf_mat),
        TransmittedShadowReceiver(),
        Transform.from_xyz(2.5, 1.0, 0.0)
            .with_rotation(Quat.from_euler(EulerRot.XYZ, 0.3, 0.5, 0.1)),
        Name("leaf"),
    )

    # Reference object behind glass (something to see through)
    ref_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.8, 0.2, 0.2),
        metallic=0.0,
        perceptual_roughness=0.5,
    ))
    ref_mesh = meshes.add(Cuboid(0.5, 0.5, 0.5))
    commands.spawn(
        Mesh3d(ref_mesh), MeshMaterial3d(ref_mat),
        Transform.from_xyz(0.0, 0.8, -2.0),
        Name("red_cube_behind"),
    )

if __name__ == "__main__":
    main().run()
```

## Key points

- **Glass:** `specular_transmission=1.0` + `ior=1.5` + `alpha_mode=AlphaMode.Blend()`
- **Frosted:** Same but with `perceptual_roughness=0.4` to scatter transmission
- **Leaf:** `diffuse_transmission=0.5` + `double_sided=True` + `cull_mode=None`
- **TransmittedShadowReceiver** shows shadows on the back side of leaves
- Place objects behind glass so refraction is visible
- Bright lighting is essential — transmission needs plenty of light to look good
