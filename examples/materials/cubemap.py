"""Reflect a procedural six-face cubemap on a sphere; rotate the sampled environment."""

from dataclasses import dataclass
from typing import Annotated

from pybevy.image import ImageSampler
from pybevy.material import Texture
from pybevy.pbr import StorageShaderMaterialPlugin
from pybevy.prelude import *
from pybevy.render import Extent3d, TextureViewDescriptor, TextureViewDimension


@material(fragment_shader="shaders/examples/cubemap.wgsl")
class CubeReflection(Material):
    environment: Annotated[Handle[Image] | None, Texture(dimension="cube")] = None
    angle: float = 0.0


@resource
@dataclass
class ReflectionSurface(Resource):
    handle: Handle[CubeReflection]


def environment_image() -> Image:
    # Cube layers follow +X, -X, +Y, -Y, +Z, -Z; each face must be square.
    colors = (
        (255, 65, 35),
        (30, 190, 255),
        (180, 120, 255),
        (20, 35, 65),
        (255, 200, 70),
        (30, 235, 130),
    )
    size = 32
    data = bytes(
        channel
        for color in colors
        for y in range(size)
        for x in range(size)
        for channel in (
            *[int(c * (0.3 + 0.7 * (x + y) / (2 * (size - 1)))) for c in color],
            255,
        )
    )
    image = Image(Extent3d(width=size, height=size, depth_or_array_layers=6), data=data)
    image.texture_view_descriptor = TextureViewDescriptor(dimension=TextureViewDimension.Cube)
    image.sampler = ImageSampler.linear()
    return image


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[CubeReflection]],
    images: ResMut[Assets[Image]],
) -> None:
    surface = materials.add(CubeReflection(environment=images.add(environment_image())))  # type: ignore[call-arg]
    commands.insert_resource(ReflectionSurface(surface))
    commands.spawn(
        Mesh3d(meshes.add(Sphere(1.25))),
        MeshMaterial3d[CubeReflection](surface),  # type: ignore[misc]
    )  # type: ignore[misc]


def setup_camera(commands: Commands) -> None:
    commands.spawn(
        Camera3d(), Transform.from_xyz(0, 0, 5).looking_at(Vec3.ZERO, Vec3.Y)
    )


def animate(
    time: Res[Time],
    state: Res[ReflectionSurface],
    materials: ResMut[Assets[CubeReflection]],
) -> None:
    surface = materials.get_mut(state.handle)
    assert surface is not None
    surface.angle = time.elapsed_secs() * 0.4


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(StorageShaderMaterialPlugin())
        .add_systems(Startup, setup)
        .add_systems(Startup, setup_camera)
        .add_systems(Update, animate)
    )


if __name__ == "__main__":
    main().run()
