"""One checker image sampled sharply on the left and smoothly on the right."""

from pybevy.image import ImageSamplerDescriptor
from pybevy.material import SamplerBinding, TextureBinding
from pybevy.mesh import MeshTag
from pybevy.pbr import ShaderMaterialPlugin
from pybevy.prelude import *
from pybevy.render import Extent3d


@material(fragment_shader="shaders/examples/independent_samplers.wgsl")
class SharedImage(Material):
    image: TextureBinding = TextureBinding(sampler=None)
    sharp: SamplerBinding = SamplerBinding("non_filtering")
    soft: SamplerBinding = SamplerBinding(default=ImageSamplerDescriptor.linear())


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[SharedImage]],
    images: ResMut[Assets[Image]],
) -> None:
    data = bytes(
        channel
        for y in range(8)
        for x in range(8)
        for channel in ((255, 170, 35, 255) if (x + y) % 2 else (25, 50, 180, 255))
    )
    image = images.add(
        Image(Extent3d(width=8, height=8, depth_or_array_layers=1), data=data)
    )
    surface = materials.add(SharedImage(image=image))
    mesh = meshes.add(Cuboid(1.8, 1.8, 0.1))
    for tag, x in enumerate((-1.0, 1.0)):
        commands.spawn(
            Mesh3d(mesh),
            MeshMaterial3d[SharedImage](surface),  # type: ignore[misc]
            MeshTag(tag),
            Transform.from_xyz(x, 0, 0),
        )  # type: ignore[misc]
    commands.spawn(
        Camera3d(), Transform.from_xyz(0, 0, 6).looking_at(Vec3.ZERO, Vec3.Y)
    )


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(ShaderMaterialPlugin())
        .add_systems(Startup, setup)
    )


if __name__ == "__main__":
    main().run()
