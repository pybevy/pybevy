"""Bevy 0.19 storage_buffer: shared colors indexed by each cube's MeshTag."""

import math
import struct
from dataclasses import dataclass

from pybevy.material import StorageBinding
from pybevy.mesh import MeshTag
from pybevy.pbr import StorageShaderMaterialPlugin
from pybevy.prelude import *
from pybevy.render import ShaderBuffer

SHADER_PATH = "shaders/examples/storage_buffer.wgsl"


@material(vertex_shader=SHADER_PATH, fragment_shader=SHADER_PATH)
class BufferedColors(Material):
    colors: StorageBinding[Vec4]


@resource
@dataclass
class ColorBuffer(Resource):
    handle: Handle[ShaderBuffer]


@system
def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[BufferedColors]],
    buffers: ResMut[Assets[ShaderBuffer]],
) -> None:
    colors = buffers.add(ShaderBuffer(struct.pack("<20f", *([1.0] * 20))))
    commands.insert_resource(ColorBuffer(colors))
    mesh = meshes.add(Cuboid(0.3, 0.3, 0.3))
    surface = materials.add(BufferedColors(colors=colors))
    for index, (x, y) in enumerate((x, y) for x in range(-6, 7) for y in range(-3, 4)):
        commands.spawn(
            Mesh3d(mesh),
            MeshMaterial3d[BufferedColors](surface),  # type: ignore[misc]
            MeshTag(index % 5),
            Transform.from_xyz(x, y, 0),
        )  # type: ignore[misc]
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0, 0, 10).looking_at(Vec3.ZERO, Vec3.Y),
    )


@system
def animate(
    time: Res[Time],
    state: Res[ColorBuffer],
    buffers: ResMut[Assets[ShaderBuffer]],
) -> None:
    buffer = buffers.get_mut(state.handle)
    assert buffer is not None
    t = time.elapsed_secs() * 5.0
    values = [
        channel
        for index in range(5)
        for channel in (
            math.sin(t + index) * 0.5 + 0.5,
            math.sin(t + index + 2) * 0.5 + 0.5,
            math.sin(t + index + 4) * 0.5 + 0.5,
            1.0,
        )
    ]
    buffer.set_data(struct.pack("<20f", *values))


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(StorageShaderMaterialPlugin())
        .add_systems(Startup, setup)
        .add_systems(Update, animate)
    )


if __name__ == "__main__":
    main().run()
