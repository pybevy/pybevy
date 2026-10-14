"""Four patterned texture layers, selected per mesh with MeshTag and a shared offset."""

from dataclasses import dataclass

from pybevy.image import ImageSampler
from pybevy.material import TextureBinding
from pybevy.mesh import MeshTag
from pybevy.pbr import ShaderMaterialPlugin
from pybevy.prelude import *
from pybevy.render import Extent3d, TextureViewDescriptor, TextureViewDimension


@material(fragment_shader="shaders/examples/array_texture.wgsl")
class LayeredTiles(Material):
    layers: TextureBinding = TextureBinding(dimension="2d_array")
    layer_offset: float = 0.0


@resource
@dataclass
class LayeredSurface(Resource):
    handle: Handle[LayeredTiles]


def layer_image() -> Image:
    colors = ((255, 65, 35), (35, 230, 100), (50, 110, 255), (255, 200, 30))
    size = 32
    data = bytes(
        channel
        for color in colors
        for y in range(size)
        for x in range(size)
        for channel in (*[c if (x // 4 + y // 4) % 2 else c // 3 for c in color], 255)
    )
    image = Image(Extent3d(width=size, height=size, depth_or_array_layers=4), data=data)
    image.texture_view_descriptor = TextureViewDescriptor(dimension=TextureViewDimension.D2Array)
    image.sampler = ImageSampler.nearest()
    return image


@system
def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[LayeredTiles]],
    images: ResMut[Assets[Image]],
) -> None:
    surface = materials.add(LayeredTiles(layers=images.add(layer_image())))  # type: ignore[call-arg]
    commands.insert_resource(LayeredSurface(surface))
    mesh = meshes.add(Cuboid(1.15, 1.15, 0.6))
    for index, (x, y) in enumerate((x, y) for y in (0.7, -0.7) for x in (-0.7, 0.7)):
        commands.spawn(
            Mesh3d(mesh),
            MeshMaterial3d[LayeredTiles](surface),  # type: ignore[misc]
            MeshTag(index),
            Transform.from_xyz(x, y, 0),
        )  # type: ignore[misc]


@system
def setup_camera(commands: Commands) -> None:
    commands.spawn(
        Camera3d(), Transform.from_xyz(0, 0, 5).looking_at(Vec3.ZERO, Vec3.Y)
    )


@system
def animate(
    time: Res[Time], state: Res[LayeredSurface], materials: ResMut[Assets[LayeredTiles]]
) -> None:
    surface = materials.get_mut(state.handle)
    assert surface is not None
    surface.layer_offset = float(int(time.elapsed_secs() / 1.5) % 4)


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
