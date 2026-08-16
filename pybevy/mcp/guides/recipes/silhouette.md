# Silhouette Traversal Recipe

Dark fog level where only rim-lit silhouettes and emissive markers define the space. Players navigate by trusting edge light and glowing waypoints.

```python
import math
from pybevy.prelude import *
from pybevy.pbr import DistanceFog, FogFalloff
from pybevy.light import VolumetricFog, VolumetricLight, FogVolume
from pybevy.color import LinearRgba


@component
class Drifter(Component):
    pass


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, drift_fog)
    )


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # --- Camera: dense fog + volumetric + bloom + desaturated grading ---
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, 3.5, 18.0).looking_at(Vec3(0.0, 2.0, -8.0), Vec3.Y),
        Bloom(intensity=0.35, low_frequency_boost=0.8),
        DistanceFog(
            color=Color.srgb(0.04, 0.04, 0.06),
            falloff=FogFalloff.Exponential(0.07),
            directional_light_color=Color.srgb(0.5, 0.55, 0.7),
            directional_light_exponent=80.0,
        ),
        VolumetricFog(
            ambient_color=Color.srgb(0.03, 0.03, 0.05),
            ambient_intensity=0.02,
            step_count=64, jitter=1.0,
        ),
        Name("camera"),
    )
    commands.insert_resource(ClearColor(Color.srgb(0.015, 0.015, 0.025)))
    commands.insert_resource(GlobalAmbientLight(brightness=15.0, color=Color.srgb(0.3, 0.35, 0.5)))

    # --- Backlight for rim silhouettes ---
    commands.spawn(
        DirectionalLight(illuminance=6000.0, color=Color.srgb(0.6, 0.7, 1.0), shadow_maps_enabled=True),
        VolumetricLight(),
        Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.3, 3.14, 0.0)),
        Name("rim_sun"),
    )

    # --- Materials ---
    silhouette_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.02, 0.02, 0.03),
        metallic=0.0, perceptual_roughness=0.15, reflectance=1.0,
    ))
    ground_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.02, 0.02, 0.025),
        perceptual_roughness=0.95, reflectance=0.1,
    ))
    marker_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.01, 0.01, 0.02),
        emissive=LinearRgba.rgb(0.4, 0.8, 2.5),
    ))
    beacon_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.02, 0.01, 0.005),
        emissive=LinearRgba.rgb(2.0, 0.8, 0.2),
    ))

    # --- Ground ---
    ground_mesh = meshes.add(Plane3d(Vec3.Y, half_size=Vec2(40.0, 40.0)))
    commands.spawn(Mesh3d(ground_mesh), MeshMaterial3d(ground_mat), Name("ground"))

    # --- Pillar corridor ---
    pillar_mesh = meshes.add(Cylinder(0.25, 5.0))
    for i in range(12):
        z = 10.0 - i * 4.0
        for x in [-3.0, 3.0]:
            commands.spawn(Mesh3d(pillar_mesh), MeshMaterial3d(silhouette_mat), Transform.from_xyz(x, 2.5, z))

    # --- Crossbeam archways ---
    beam_mesh = meshes.add(Cuboid(6.0, 0.2, 0.3))
    for i in range(0, 12, 2):
        z = 10.0 - i * 4.0
        commands.spawn(Mesh3d(beam_mesh), MeshMaterial3d(silhouette_mat), Transform.from_xyz(0.0, 5.0, z))

    # --- Emissive path markers ---
    strip_mesh = meshes.add(Cuboid(0.15, 0.04, 0.8))
    for i in range(16):
        z = 12.0 - i * 3.0
        for x in [-1.2, 1.2]:
            commands.spawn(Mesh3d(strip_mesh), MeshMaterial3d(marker_mat), Transform.from_xyz(x, 0.02, z))

    # --- Amber waypoint beacons ---
    beacon_mesh = meshes.add(Sphere(0.12).mesh().ico(3))
    for z in [8.0, 0.0, -8.0, -16.0, -24.0, -32.0]:
        commands.spawn(Mesh3d(beacon_mesh), MeshMaterial3d(beacon_mat), Transform.from_xyz(0.0, 1.0, z))

    # --- Drifting fog volumes ---
    for i in range(4):
        z = 6.0 - i * 10.0
        commands.spawn(
            FogVolume(
                density_factor=0.15, fog_color=Color.srgb(0.06, 0.06, 0.1),
                absorption=0.4, scattering=0.6, scattering_asymmetry=0.7,
            ),
            Transform.from_xyz(0.0, 1.5, z).with_scale(Vec3(8.0, 3.0, 6.0)),
            Drifter(), Name(f"fog_volume_{i}"),
        )

    # --- Rim accent lights behind key silhouettes ---
    for lx, ly, lz in [(0, 3, -10), (0, 3, -24)]:
        commands.spawn(
            PointLight(intensity=60000.0, color=Color.srgb(0.5, 0.6, 1.0), range=14.0, shadow_maps_enabled=True),
            VolumetricLight(),
            Transform.from_xyz(float(lx), float(ly), float(lz)),
        )


def drift_fog(query: Query[Mut[Transform], With[Drifter]], time: Res[Time]) -> None:
    t = time.elapsed_secs()
    idx = 0
    for transform in query:
        phase = idx * 1.7
        transform.translation.x = math.sin(t * 0.12 + phase) * 3.0
        transform.translation.y = 1.5 + math.sin(t * 0.08 + phase * 0.5) * 0.5
        idx += 1


if __name__ == "__main__":
    main().run()
```

## Key techniques

- **Silhouette material**: near-black `base_color` + `reflectance=1.0` + low `roughness` → only rim edges catch backlight via Fresnel
- **Dark fog**: `color` near-black (not gray), density 0.07, `ClearColor` matching fog color
- **Dual navigation cues**: cool blue ground strips (path edges) + warm amber beacons (waypoints)
- **Volumetric fog volumes**: `FogVolume` entities with `Drifter` component drift laterally for living atmosphere

## Tuning visibility

- **More visible**: lower fog density (0.04), raise ambient brightness (30–50), increase emissive values
- **More oppressive**: raise fog density (0.09+), reduce emissive to 0.5–1.0 range, drop ambient to 5
- **Ground visibility**: if players need to see the floor, add a subtle ground emissive or raise ambient to 30+
