"""Sweep through a procedural 3D texture with trilinear filtering."""

import math
from dataclasses import dataclass

from pybevy.image import ImageSampler
from pybevy.material import TextureBinding
from pybevy.pbr import ShaderMaterialPlugin
from pybevy.prelude import *
from pybevy.render import (
    Extent3d,
    TextureDimension,
    TextureViewDescriptor,
    TextureViewDimension,
)


@material(fragment_shader="shaders/examples/volume_texture.wesl")
class VolumeSlice(Material):
    volume: TextureBinding = TextureBinding(dimension="3d")
    slice_z: float = 0.15


@resource
@dataclass
class VolumeSurface(Resource):
    handle: Handle[VolumeSlice]


def volume_image() -> Image:
    size = 32
    data = bytearray()
    # X varies fastest, then Y, then Z. D3 stores a volume, not array layers.
    for z in range(size):
        for y in range(size):
            for x in range(size):
                wave = 0.5 + 0.5 * math.sin(x * 0.35 + math.cos(y * 0.3) * 2 + z * 0.25)
                data.extend(
                    (
                        int(255 * wave),
                        int(255 * z / (size - 1)),
                        int(255 * (1 - wave)),
                        255,
                    )
                )
    image = Image(
        Extent3d(width=size, height=size, depth_or_array_layers=size),
        dimension=TextureDimension.D3,
        data=bytes(data),
    )
    image.texture_view_descriptor = TextureViewDescriptor(dimension=TextureViewDimension.D3)
    image.sampler = ImageSampler.linear()
    return image


@system
def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[VolumeSlice]],
    images: ResMut[Assets[Image]],
) -> None:
    surface = materials.add(VolumeSlice(volume=images.add(volume_image())))  # type: ignore[call-arg]
    commands.insert_resource(VolumeSurface(surface))
    commands.spawn(
        Mesh3d(meshes.add(Cuboid(2.6, 2.6, 0.1))),
        MeshMaterial3d[VolumeSlice](surface),  # type: ignore[misc]
    )  # type: ignore[misc]


@system
def setup_camera(commands: Commands) -> None:
    commands.spawn(
        Camera3d(), Transform.from_xyz(0, 0, 5).looking_at(Vec3.ZERO, Vec3.Y)
    )


@system
def animate(
    time: Res[Time], state: Res[VolumeSurface], materials: ResMut[Assets[VolumeSlice]]
) -> None:
    surface = materials.get_mut(state.handle)
    assert surface is not None
    surface.slice_z = 0.5 + 0.45 * math.sin(time.elapsed_secs() * 0.7)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(ShaderMaterialPlugin())
        .add_systems(Startup, setup)
        .add_systems(Startup, setup_camera)
        .add_systems(Update, animate)
    )


if __name__ == "__main__":
    main().run()
