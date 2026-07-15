# Volumetric Fog Scene Recipe

Complete scene with god rays, atmosphere, and a localized fog volume.

```python
import math
from pybevy.prelude import *
from pybevy.light import (
    ScatteringMedium, Atmosphere,
    VolumetricFog, VolumetricLight, FogVolume,
    AtmosphereEnvironmentMapLight, SunDisk,
)
from pybevy.pbr import AtmosphereSettings

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
    mediums: ResMut[Assets[ScatteringMedium]],
) -> None:
    medium_handle = mediums.add(ScatteringMedium.earth())

    # The atmosphere (planet) lives on its own entity
    commands.spawn(Atmosphere.earth(medium_handle))

    # Camera with volumetric fog; AtmosphereSettings opts it into the sky
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(8, 4, 8).looking_at(Vec3(0, 1, 0), Vec3.Y),
        Bloom(intensity=0.2, low_frequency_boost=0.5),
        AtmosphereSettings(),  # sky won't render for this camera without it
        AtmosphereEnvironmentMapLight(intensity=0.8),
        VolumetricFog(
            ambient_color=Color.srgb(0.05, 0.05, 0.08),
            ambient_intensity=0.1,
            step_count=64,
            jitter=1.0,
        ),
        Name("camera"),
    )

    # Sun with god rays
    commands.spawn(
        DirectionalLight(illuminance=12000.0, shadow_maps_enabled=True),
        VolumetricLight(),
        SunDisk.EARTH,
        Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.6, 0.3, 0.0)),
        Name("sun"),
    )

    # Ambient fill
    commands.insert_resource(GlobalAmbientLight(brightness=100.0))

    # Ground
    ground_mesh = meshes.add(Plane3d(Vec3.Y, half_size=Vec2(15.0, 15.0)))
    ground_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.25, 0.35, 0.15),
    ))
    commands.spawn(Mesh3d(ground_mesh), MeshMaterial3d(ground_mat), Name("ground"))

    # Localized fog volume (swamp mist near the ground)
    # NOTE: Keep absorption low for bright mist. High absorption = dark fog.
    # See guide://lighting (FogVolume parameter reference) for details.
    commands.spawn(
        FogVolume(
            density_factor=0.2,
            fog_color=Color.srgb(0.5, 0.55, 0.5),
            absorption=0.05,
            scattering=0.5,
            scattering_asymmetry=0.3,
        ),
        Transform.from_xyz(0, 0.5, 0).with_scale(Vec3(10.0, 1.0, 10.0)),
        Name("mist"),
    )

    # Some columns to catch god rays
    pillar_mesh = meshes.add(Cylinder(0.3, 4.0))
    pillar_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.5, 0.45, 0.4),
    ))
    for i in range(6):
        angle = i * math.pi / 3.0
        commands.spawn(
            Mesh3d(pillar_mesh),
            MeshMaterial3d(pillar_mat),
            Transform.from_xyz(math.cos(angle) * 4.0, 2.0, math.sin(angle) * 4.0),
        )

if __name__ == "__main__":
    main().run()
```

## Key points

- **VolumetricFog** on camera enables the volumetric pass
- **VolumetricLight** on the directional light makes it create god rays
- **FogVolume** creates localized fog - `Transform` scale controls its size
- **jitter=1.0** reduces banding artifacts (best with TAA)
- Columns/geometry break up the light for visible shafts
- **absorption** should be low (0.02–0.05) for bright white mist. Higher values (0.2+) create dark/smoky fog - see `guide://lighting` for the full parameter reference
- **Performance:** cost scales with `step_count`, resolution, and the number of shadowed `VolumetricLight`s. Large fog volumes with several shadowed lights run below 30 FPS on mid-range GPUs; prefer `step_count=16-32` and one volumetric light, and check `get_performance` after enabling
