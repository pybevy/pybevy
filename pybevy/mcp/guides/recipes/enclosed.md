# Enclosed Chamber Recipe

Sealed room with walls, floor, ceiling, colored point lights, emissive accents, and fog. A good starting point for dungeons, sci-fi chambers, arenas, or any fully enclosed space.

**Key difference from the basic indoor recipe:** enclosed scenes need **brighter ambient + more point lights** because there's no sky/sun contribution — all light must come from placed sources.

```python
from pybevy.prelude import *
from pybevy.color import LinearRgba
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
    # Camera
    # Low bloom for enclosed spaces — high bloom blows out emissive objects
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(8, 6, 8).looking_at(Vec3(0, 1.5, 0), Vec3.Y),
        Bloom(intensity=0.08, low_frequency_boost=0.2),
        # Light fog — keep density LOW indoors (0.003–0.005)
        DistanceFog(
            color=Color.srgb(0.04, 0.03, 0.08),
            falloff=FogFalloff.Exponential(0.004),
            directional_light_color=Color.srgb(0.4, 0.2, 0.7),
            directional_light_exponent=30.0,
        ),
        Name("camera"),
    )

    # Lighting
    # Directional still helps even indoors — gives general fill through walls
    commands.spawn(
        DirectionalLight(
            illuminance=8000.0,
            color=Color.srgb(0.6, 0.55, 0.8),
            shadow_maps_enabled=True,
        ),
        Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.7, 0.5, 0.0)),
        Name("sun"),
    )
    # Higher ambient than outdoor — no sky bounce to fill shadows
    commands.insert_resource(GlobalAmbientLight(
        brightness=500.0,
        color=Color.srgb(0.4, 0.35, 0.6),
    ))
    commands.insert_resource(ClearColor(Color.srgb(0.02, 0.02, 0.05)))

    # Room dimensions
    HALF_W = 6.0   # half-width
    WALL_H = 5.0   # wall height

    # Meshes
    floor_mesh = meshes.add(Plane3d(Vec3.Y, half_size=Vec2(HALF_W, HALF_W)))
    ceiling_mesh = meshes.add(Plane3d(Vec3(0.0, -1.0, 0.0), half_size=Vec2(HALF_W, HALF_W)))
    wall_long = meshes.add(Cuboid(HALF_W * 2, WALL_H, 0.3))
    wall_side = meshes.add(Cuboid(0.3, WALL_H, HALF_W * 2))

    # Materials
    # Start with visible base colors (0.15–0.25 range), not near-black
    floor_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.18, 0.16, 0.24),
        metallic=0.85, perceptual_roughness=0.25,
    ))
    wall_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.2, 0.18, 0.28),
        metallic=0.7, perceptual_roughness=0.4,
    ))
    ceiling_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.14, 0.12, 0.22),
        metallic=0.6, perceptual_roughness=0.5,
    ))

    # Room geometry
    commands.spawn(Mesh3d(floor_mesh), MeshMaterial3d(floor_mat), Name("floor"))
    commands.spawn(Mesh3d(ceiling_mesh), MeshMaterial3d(ceiling_mat),
                   Transform.from_xyz(0, WALL_H, 0), Name("ceiling"))
    commands.spawn(Mesh3d(wall_long), MeshMaterial3d(wall_mat),
                   Transform.from_xyz(0, WALL_H / 2, HALF_W), Name("wall_n"))
    commands.spawn(Mesh3d(wall_long), MeshMaterial3d(wall_mat),
                   Transform.from_xyz(0, WALL_H / 2, -HALF_W), Name("wall_s"))
    commands.spawn(Mesh3d(wall_side), MeshMaterial3d(wall_mat),
                   Transform.from_xyz(HALF_W, WALL_H / 2, 0), Name("wall_e"))
    commands.spawn(Mesh3d(wall_side), MeshMaterial3d(wall_mat),
                   Transform.from_xyz(-HALF_W, WALL_H / 2, 0), Name("wall_w"))

    # Accent emissive object
    # With unlit=True, base_color IS the visible surface — set it BRIGHT
    glow_mesh = meshes.add(Sphere(0.5).mesh().ico(4))
    glow_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.6, 0.2, 0.9),      # Visible bright purple
        emissive=LinearRgba.rgb(10.0, 2.0, 16.0),   # Bloom halo
        unlit=True,
    ))
    commands.spawn(
        Mesh3d(glow_mesh), MeshMaterial3d(glow_mat),
        Transform.from_xyz(0, 2.0, 0),
        Name("glow_orb"),
    )

    # Point lights — primary illumination for enclosed spaces
    # Central accent light (matches the emissive object color)
    commands.spawn(
        PointLight(intensity=500000.0, color=Color.srgb(0.6, 0.2, 1.0),
                   range=18.0, shadow_maps_enabled=True),
        Transform.from_xyz(0, 3.5, 0), Name("light_center"),
    )
    # Corner fill lights — use 4 to eliminate dark pockets
    for x, z in [(-4, -4), (-4, 4), (4, -4), (4, 4)]:
        commands.spawn(
            PointLight(intensity=200000.0, color=Color.srgb(0.5, 0.7, 1.0), range=14.0),
            Transform.from_xyz(float(x), WALL_H - 0.5, float(z)),
        )

    # Scene content placeholder
    box_mesh = meshes.add(Cuboid(1.0, 1.0, 1.0))
    box_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.7, 0.45, 0.1),
        metallic=0.9, perceptual_roughness=0.2,
    ))
    commands.spawn(
        Mesh3d(box_mesh), MeshMaterial3d(box_mat),
        Transform.from_xyz(3.0, 0.5, -2.0), Name("crate"),
    )

if __name__ == "__main__":
    main().run()
```

## Key differences from outdoor scenes

| Setting | Outdoor | Enclosed chamber |
|---------|---------|-----------------|
| GlobalAmbientLight brightness | 200–400 | **400–600** (no sky bounce) |
| DirectionalLight | Primary light source | Fill only (passes through walls) |
| PointLights | Optional accents | **Primary illumination** — use 4+ |
| PointLight intensity | 50k–100k | **150k–500k** (must light the room alone) |
| Fog density | 0.002 | **0.003–0.005** (lower — walls are close) |
| Bloom intensity | 0.15–0.25 | **0.05–0.12** (emissive objects are closer) |
| Material base_color | 0.05–0.15 ok | **0.15–0.30** (brighter to catch limited light) |

## Common mistakes in enclosed scenes

1. **Too dark** — The #1 issue. Enclosed spaces have no sky/sun contribution. Start with ambient 500, 4+ point lights at 200k+, and material base_colors above 0.15. Brighten first, dim later.
2. **Emissive objects invisible** — With `unlit=True`, a dark `base_color` makes the mesh a silhouette. Set `base_color` to the bright version of your glow color (e.g., `0.6, 0.2, 0.9` for purple glow).
3. **Bloom blowout** — Emissive objects are close to the camera indoors. Use bloom intensity 0.05–0.12 and emissive values 3–15. Thin meshes (torus, thin cylinders) bloom into blobs faster than solid shapes.
4. **Fog too thick** — Walls are only 10–15 units away. Density above 0.005 makes everything hazy. Start at 0.003.
