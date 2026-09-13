# Decals Recipe

Project a texture onto existing geometry using clustered decals.

```python
from pybevy.prelude import *
from pybevy.light import ClusteredDecal
from pybevy.render import Extent3d

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
    images: ResMut[Assets[Image]],
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

    # A tiny procedural RGBA texture keeps the recipe self-contained.
    decal_pixels = [
        0, 0, 0, 0,       220, 35, 25, 255,  0, 0, 0, 0,
        220, 35, 25, 255, 255, 210, 80, 255, 220, 35, 25, 255,
        0, 0, 0, 0,       220, 35, 25, 255,  0, 0, 0, 0,
    ]
    decal_texture = images.add(Image(Extent3d(width=3, height=3, depth_or_array_layers=1), data=decal_pixels))

    # Decal volume
    commands.spawn(
        ClusteredDecal(base_color_texture=decal_texture),
        Transform.from_xyz(0.0, 1.5, -0.8),
        Name("decal"),
    )

if __name__ == "__main__":
    main().run()
```

## Key points

- Clustered decals require bindless textures. Bevy disables them on WebGL 2,
  WebGPU, macOS, iOS, and adapters without binding-array support; on those
  targets the component can exist but renders no decal. Use the forward-decal
  approach below when portability matters.
- **ClusteredDecal** applies its texture inside a box centred on the entity, extending
  `Transform.scale / 2` along each local axis. It is not a projector throwing forward.
- **A clustered decal's `Transform.scale` decides whether it appears at all.** The surface must
  fall inside that box, so `scale.z` must exceed twice the standoff distance: a decal 2.0 above
  a floor needs `scale.z > 4.0`. Falling short renders nothing, with no error. The example
  above works because its 0.1 surface gap fits the default `scale.z` of 1.0 (half-depth 0.5).
  Forward decals are flat quads and ignore this rule.
- Rotation aims the box; position centres it
- Use `tag` field to group decals (e.g., `tag=1` for blood, `tag=2` for bullet holes)
- The texture should have an alpha channel for the decal shape

## Forward decals

Forward decals are mesh-based and use a material rather than a volume texture.
`PbrPlugin` registers their renderer. `ForwardDecal` fills `Mesh3d` with Bevy's
own 1x1 quad, lying in the local XZ plane and facing +Y: `scale.x` and `scale.z`
size the footprint, `scale.y` does nothing, and the quad fades out over
`depth_fade_factor` metres of gap, so keep it just above the surface it marks.

```python
from pybevy.assets import Assets
from pybevy.camera import DepthPrepass
from pybevy.ecs import Commands, ResMut
from pybevy.image import Image
from pybevy.math import Vec3
from pybevy.mesh import MeshMaterial3d
from pybevy.pbr import (
    ForwardDecal,
    ForwardDecalMaterial,
    ForwardDecalMaterialExt,
    StandardMaterial,
)
from pybevy.render import Extent3d
from pybevy.transform import Transform

def setup_forward_decal(
    commands: Commands,
    decal_materials: ResMut[Assets[ForwardDecalMaterial]],
    images: ResMut[Assets[Image]],
) -> None:
    texture = images.add(Image(Extent3d(width=1, height=1, depth_or_array_layers=1), data=[220, 35, 25, 255]))
    material = decal_materials.add(ForwardDecalMaterial(
        base=StandardMaterial(
            base_color_texture=texture,
        ),
        extension=ForwardDecalMaterialExt(depth_fade_factor=1.0),
    ))
    commands.spawn(
        ForwardDecal(),
        MeshMaterial3d[ForwardDecalMaterial](material),
        Transform.from_xyz(0.0, 0.05, 0.0).with_scale(Vec3(4.0, 1.0, 4.0)),
    )

# Include DepthPrepass() when spawning the Camera3d entity.
```

Two mistakes render nothing and log only a shader error: a camera without
`DepthPrepass` leaves `prepass_depth` undefined, and an explicit `Mesh3d` without
tangents leaves `world_tangent` invalid; build that mesh with
`with_generated_tangents()`.
